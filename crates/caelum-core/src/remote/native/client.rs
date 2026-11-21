use super::{
    protocol::{self, CommandFrame, ServerFrame, VERSION},
    ssh, NativeTarget,
};
use crate::application::{Command, Event};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, watch},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Connecting,
    Connected,
    Disconnected(String),
}

pub enum Notice {
    Status(Status),
    Event(Box<Event>),
}

struct Active {
    writer: mpsc::Sender<CommandFrame>,
    pending: BTreeSet<u64>,
}
type Link = Arc<Mutex<Option<Active>>>;
enum Message {
    Connected(Link),
    Notice(Notice),
}
struct Output {
    tx: mpsc::Sender<Message>,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Output {
    async fn send(&self, n: Notice) {
        self.publish(Message::Notice(n)).await;
    }
    async fn publish(&self, message: Message) {
        if self.tx.send(message).await.is_ok() {
            (self.wake)()
        }
    }
}

/// A connection accepts commands only after its exact peer has identified itself. No outbox
/// survives a tunnel loss. An uncertain mutation is reconciled from native history, never retried.
pub struct Client {
    root: String,
    /// Only the connection whose Connected notice the desktop has observed can accept commands.
    link: Option<Link>,
    rx: mpsc::Receiver<Message>,
    retry: mpsc::Sender<()>,
    stop: watch::Sender<bool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Client {
    pub fn start(
        target: NativeTarget,
        parent_store: PathBuf,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let root = target.root()?;
        Self::with_tunnel(root, target.repository.clone(), wake, async move || {
            ssh::open(&target, &parent_store).await
        })
    }

    pub(super) fn with_tunnel(
        root: String,
        repository: String,
        wake: impl Fn() + Send + Sync + 'static,
        mut open: impl AsyncFnMut() -> Result<ssh::Tunnel, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::with_connection(root.clone(), wake, async move |link, out| {
            let mut tunnel = match open().await {
                Ok(tunnel) => tunnel,
                Err(e) => return e,
            };
            let problem = session(
                &root,
                &repository,
                link,
                out,
                &mut tunnel.child.stdout.take().expect("piped stdout"),
                &mut tunnel.child.stdin.take().expect("piped stdin"),
            )
            .await;
            let status = tokio::time::timeout(Duration::from_millis(300), tunnel.child.wait())
                .await
                .ok()
                .and_then(Result::ok)
                .and_then(|status| status.code());
            match status {
                Some(66) => "Could not install the matching Caelum workspace peer from the Linux release. Check the VM’s network and desktop version.".into(),
                Some(65) => "The native workspace needs an x86_64 or aarch64 Linux VM.".into(),
                Some(255) => "The verified SSH connection closed. Check that the VM is running, then reconnect to refresh its address and host key.".into(),
                _ => problem,
            }
        })
    }

    fn with_connection(
        root: String,
        wake: impl Fn() + Send + Sync + 'static,
        connect: impl AsyncFnMut(&Link, &Output) -> String + Send + 'static,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel(512);
        let (retry, again) = mpsc::channel(1);
        let (stop, mut closing) = watch::channel(false);
        let wake = Arc::new(wake);
        let out = Output {
            tx,
            wake: wake.clone(),
        };
        std::thread::Builder::new()
            .name("native-workspace-ssh".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match rt {
                    Ok(rt) => rt.block_on(async {
                        tokio::select! {
                            _ = closing.changed()=>{},
                            _ = supervise(connect,again,&out)=>{},
                        }
                    }),
                    Err(e) => {
                        let _ = out.tx.blocking_send(Message::Notice(Notice::Status(
                            Status::Disconnected(e.to_string()),
                        )));
                        (out.wake)();
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            link: None,
            rx,
            retry,
            stop,
            wake,
        })
    }

    pub fn submit(&self, id: u64, command: Command) -> Result<(), String> {
        if !protocol::admitted(&command, &self.root) {
            return Err("This action is not available in a native VM workspace.".into());
        }
        let offline = "The VM is disconnected. Reconnect before trying again.";
        let mut guard = self
            .link
            .as_ref()
            .ok_or(offline)?
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let active = guard.as_mut().ok_or(offline)?;
        if active.pending.len() >= 512 || !active.pending.insert(id) {
            return Err("Too many pending native workspace requests.".into());
        }
        if active.writer.try_send(CommandFrame(id, command)).is_err() {
            active.pending.remove(&id);
            return Err("The VM connection is busy or closed. Try again.".into());
        }
        Ok(())
    }
    pub fn reconnect(&self) {
        let _ = self.retry.try_send(());
    }
    pub fn drain(&mut self) -> Vec<Notice> {
        let notices: Vec<_> = std::iter::from_fn(|| self.rx.try_recv().ok())
            .take(256)
            .map(|m| match m {
                Message::Connected(link) => {
                    self.link = Some(link);
                    Notice::Status(Status::Connected)
                }
                Message::Notice(n) => {
                    if matches!(n, Notice::Status(_)) {
                        self.link = None;
                    }
                    n
                }
            })
            .collect();
        if notices.len() == 256 {
            (self.wake)();
        }
        notices
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

async fn supervise(
    mut connect: impl AsyncFnMut(&Link, &Output) -> String,
    mut again: mpsc::Receiver<()>,
    out: &Output,
) {
    loop {
        out.send(Notice::Status(Status::Connecting)).await;
        let link = Link::default();
        let problem = connect(&link, out).await;
        let retired = link.lock().unwrap_or_else(|e| e.into_inner()).take();
        out.send(Notice::Status(Status::Disconnected(problem)))
            .await;
        if let Some(retired) = retired {
            for id in retired.pending {
                let error = "VM connection lost. Delivery is unconfirmed; check the session history before retrying.";
                out.send(Notice::Event(Box::new(Event::Completed(
                    id,
                    Err(error.into()),
                ))))
                .await;
            }
            // Try a fresh, independently authenticated connection once; a stopped VM or revoked
            // credential then waits for the user's Reconnect action rather than polling Boat.
            tokio::select! { _=tokio::time::sleep(Duration::from_secs(1))=>{}, _=again.recv()=>{} }
        } else if again.recv().await.is_none() {
            return;
        }
    }
}

async fn frame(input: &mut (impl AsyncRead + Unpin)) -> Result<ServerFrame, String> {
    let n = input
        .read_u32()
        .await
        .map_err(|_| "Native VM connection closed. Check the VM checkout and installed peer.")?
        as usize;
    if n == 0 || n > protocol::LIMIT {
        return Err("Invalid native VM frame size.".into());
    }
    let mut data = vec![0; n];
    input
        .read_exact(&mut data)
        .await
        .map_err(|_| "Incomplete native VM response.")?;
    serde_json::from_slice(&data).map_err(|_| "Incompatible native VM response.".into())
}
async fn session(
    expected_root: &str,
    expected_repo: &str,
    link: &Link,
    out: &Output,
    input: &mut (impl AsyncRead + Unpin),
    output: &mut (impl AsyncWrite + Unpin),
) -> String {
    // Broker snapshots precede Connected so a stale question/permission cannot be answered
    // during the gap between a new handshake and recovery of its pending requests.
    let handshake = async {
        match frame(input).await? {
            ServerFrame::Ready {
                version,
                root,
                repository,
            } if version == VERSION
                && root == expected_root
                && repository.eq_ignore_ascii_case(expected_repo) => {}
            ServerFrame::Error(e) => return Err(e),
            _ => {
                return Err(
                    "The workspace peer has a different protocol version or checkout.".into(),
                )
            }
        }
        match (frame(input).await?, frame(input).await?) {
            (ServerFrame::Event(a), ServerFrame::Event(q))
                if matches!(*a, Event::Approvals(_)) && matches!(*q, Event::Questions(_)) =>
            {
                Ok([a, q])
            }
            _ => Err("The workspace peer did not restore its pending requests.".into()),
        }
    };
    match tokio::time::timeout(Duration::from_secs(125), handshake).await {
        Ok(Ok(snapshots)) => {
            for event in snapshots {
                out.send(Notice::Event(event)).await;
            }
            exchange(link, out, input, output).await
        }
        Ok(Err(e)) => e,
        Err(_) => "Timed out preparing the native workspace on the VM.".into(),
    }
}

async fn exchange(
    link: &Link,
    out: &Output,
    input: &mut (impl AsyncRead + Unpin),
    output: &mut (impl AsyncWrite + Unpin),
) -> String {
    let (writer, mut rx) = mpsc::channel(128);
    *link.lock().unwrap_or_else(|e| e.into_inner()) = Some(Active {
        writer,
        pending: BTreeSet::new(),
    });
    out.publish(Message::Connected(link.clone())).await;
    let read = async {
        loop {
            match frame(input).await {
                Ok(ServerFrame::Event(event)) => {
                    let reply = if let Event::Completed(id, _) = event.as_ref() {
                        if !link
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .as_ref()
                            .is_some_and(|a| a.pending.contains(id))
                        {
                            continue;
                        }
                        Some(*id)
                    } else {
                        None
                    };
                    out.send(Notice::Event(event)).await;
                    if let Some(id) = reply {
                        if let Some(active) =
                            link.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
                        {
                            active.pending.remove(&id);
                        }
                    }
                }
                Ok(_) => return "Unexpected native VM handshake.".into(),
                Err(e) => return e,
            }
        }
    };
    let write = async {
        while let Some(request) = rx.recv().await {
            let data = match protocol::encode(&request) {
                Ok(data) => data,
                Err(e) => {
                    let id = request.0;
                    out.send(Notice::Event(Box::new(Event::Completed(
                        id,
                        Err(e.to_string()),
                    ))))
                    .await;
                    if let Some(active) = link.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                        active.pending.remove(&id);
                    }
                    continue;
                }
            };
            if output.write_all(&data).await.is_err() || output.flush().await.is_err() {
                return "Could not write to the VM connection.".into();
            }
        }
        "Native VM workspace closed.".into()
    };
    tokio::select! { error=read=>error, error=write=>error }
}

#[cfg(test)]
mod tests;

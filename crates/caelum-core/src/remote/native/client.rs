use super::{commands, ssh, NativeTarget};
use crate::application::{Command, Event, ProviderRuntime, Runtime};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, watch, Notify};

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
    writer: mpsc::Sender<(u64, Command)>,
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

/// A connection accepts commands only after SSH verifies the checkout. No outbox
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
        Self::with_tunnel(root, wake, async move || {
            ssh::open(&target, &parent_store).await
        })
    }

    pub(super) fn with_tunnel(
        root: String,
        wake: impl Fn() + Send + Sync + 'static,
        mut open: impl AsyncFnMut() -> Result<ssh::Tunnel, String> + Send + 'static,
    ) -> Result<Self, String> {
        let mut recover_open = false;
        Self::with_connection(root, wake, async move |link, out| {
            let mut tunnel = match open().await {
                Ok(tunnel) => tunnel,
                Err(e) => return e,
            };
            let access = tunnel.access.clone();
            let problem = tokio::select! {
                _=tunnel.child.wait()=>"SSH disconnected. Reconnect to verify the VM’s address and host key.".to_owned(),
                e=session(access.clone(),recover_open,link,out)=>e,
            };
            recover_open = access
                .opencode_used
                .load(std::sync::atomic::Ordering::Acquire);
            problem
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
        if !commands::admitted(&command, &self.root) {
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
        if active.writer.try_send((id, command)).is_err() {
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

// Runtime shutdown joins blocking provider workers; a dropped SSH attempt must
// release its master immediately, including while a read is still in progress.
struct Workers(Option<Runtime>, Arc<super::Access>);
impl Drop for Workers {
    fn drop(&mut self) {
        self.1
            .live
            .store(false, std::sync::atomic::Ordering::Release);
        if let Some(runtime) = self.0.take() {
            std::thread::spawn(move || drop(runtime));
        }
    }
}
fn activate(link: &Link) -> mpsc::Receiver<(u64, Command)> {
    let (writer, rx) = mpsc::channel(128);
    *link.lock().unwrap_or_else(|e| e.into_inner()) = Some(Active {
        writer,
        pending: BTreeSet::new(),
    });
    rx
}
async fn deliver(link: &Link, out: &Output, event: Event) {
    let completion = if let Event::Completed(id, _) = &event {
        if !link
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|a| a.pending.contains(id))
        {
            return;
        }
        Some(*id)
    } else {
        None
    };
    out.send(Notice::Event(Box::new(event))).await;
    if let Some(id) = completion {
        if let Some(active) = link.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            active.pending.remove(&id);
        }
    }
}
async fn session(access: Arc<super::Access>, recover: bool, link: &Link, out: &Output) -> String {
    let wake = Arc::new(Notify::new());
    let notify = wake.clone();
    let providers = ProviderRuntime::forwarded(access.clone());
    let mut workers = Workers(
        Some(Runtime::with_providers(providers.clone(), move || {
            notify.notify_one()
        })),
        access.clone(),
    );
    let rt = workers.0.as_mut().unwrap();
    if recover {
        let opencode = providers.opencode.clone();
        let root = access.root.clone();
        match tokio::task::spawn_blocking(move || opencode.attach(&root)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return e.to_string(),
            Err(e) => return e.to_string(),
        }
    }
    out.send(Notice::Event(Box::new(Event::Approvals(
        providers.approvals.pending(),
    ))))
    .await;
    out.send(Notice::Event(Box::new(Event::Questions(
        providers.questions.pending(),
    ))))
    .await;
    let mut requests = activate(link);
    out.publish(Message::Connected(link.clone())).await;
    loop {
        tokio::select! {
            _ = wake.notified() => for event in rt.drain() {
                let event = match event {
                    Event::OpenCode(ref e) if e.event_type == "server.disconnected"
                        && access.opencode_used.load(std::sync::atomic::Ordering::Acquire) => return "VM OpenCode event stream disconnected.".into(),
                    Event::Codex(ref e) if e.method == "codex/processExited"
                        && access.codex_used.load(std::sync::atomic::Ordering::Acquire) => return "VM Codex WebSocket disconnected.".into(),
                    Event::Approvals(_) => Event::Approvals(providers.approvals.pending()),
                    Event::Questions(_) => Event::Questions(providers.questions.pending()),
                    e => e,
                };
                deliver(link, out, event).await;
            },
            req = requests.recv() => match req {
                Some((id, cmd)) => if let Err(e) = rt.submit(id, cmd) {
                    deliver(link, out, Event::Completed(id, Err(e))).await;
                },
                None => return "VM workspace closed.".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests;

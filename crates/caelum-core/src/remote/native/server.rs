//! A single, checkout-owned provider runtime outlives SSH connections. No TCP listener.
use super::protocol::{admitted, read_frame, write_frame, CommandFrame, ServerFrame, VERSION};
use crate::application::{Event, ProviderRuntime, Runtime};
use crate::remote::{checkout::repo_name, github_slug};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        fs::DirBuilderExt,
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Entry point of `caelum workspace-peer`. stdout is exclusively framed data in attach mode.
pub fn run_peer(mode: &str, repository: &str) -> Result<(), String> {
    let name = repo_name(repository)?;
    let home = std::env::var_os("HOME").ok_or("No VM home directory")?;
    let home = PathBuf::from(home).canonicalize().map_err(error)?;
    let root = home
        .join(name)
        .into_os_string()
        .into_string()
        .map_err(|_| "Invalid VM checkout path")?;
    if let Err(e) = verify(&root, repository) {
        if mode == "attach" {
            let _ = write_frame(&mut io::stdout().lock(), &ServerFrame::Error(e.clone()));
        }
        return Err(e);
    }
    let dir = home.join(".local/state/caelum/peers");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(error)?;
    let hash = super::digest(repository.to_ascii_lowercase().as_bytes());
    let socket = dir.join(format!("v{VERSION}-{}.sock", &hash[..20]));
    match mode {
        "attach" => attach(&socket, repository),
        "serve" => serve(&socket, root, repository.into()),
        _ => Err("Expected workspace-peer attach or serve".into()),
    }
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn verify(root: &str, repository: &str) -> Result<(), String> {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
    };
    if git(&["rev-parse", "--show-toplevel"]).is_none_or(|s| s.trim_end() != root)
        || git(&["remote", "get-url", "origin"])
            .and_then(|s| github_slug(s.trim()))
            .is_none_or(|s| !s.eq_ignore_ascii_case(repository))
    {
        return Err("The VM checkout does not match the selected GitHub repository.".into());
    }
    Ok(())
}

fn attach(socket: &Path, repository: &str) -> Result<(), String> {
    let mut connection = match UnixStream::connect(socket) {
        Ok(c) => c,
        Err(_) => {
            let mut daemon = Command::new(std::env::current_exe().map_err(error)?)
                .args(["workspace-peer", "serve", repository])
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(error)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Ok(c) = UnixStream::connect(socket) {
                    break c;
                }
                if Instant::now() >= deadline {
                    let _ = daemon.kill();
                    let _ = daemon.wait();
                    return Err("Could not start the native VM workspace.".into());
                }
                thread::sleep(Duration::from_millis(40));
            }
        }
    };
    let mut out = connection.try_clone().map_err(error)?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut out);
        let _ = out.shutdown(Shutdown::Write);
    });
    // Stdout is line-buffered, but a JSON frame need not contain a newline. Flush
    // each socket read; otherwise Ready and small interactive events can wait forever.
    let mut stdout = io::stdout().lock();
    let mut buffer = [0; 16 * 1024];
    loop {
        let n = connection.read(&mut buffer).map_err(error)?;
        if n == 0 {
            return Ok(());
        }
        stdout
            .write_all(&buffer[..n])
            .and_then(|_| stdout.flush())
            .map_err(error)?;
    }
}

enum Action {
    Attach(UnixStream),
    Request(u64, Box<CommandFrame>),
    Closed(u64),
    Wake,
    Shutdown,
}

fn serve(socket: &Path, root: String, repository: String) -> Result<(), String> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(socket.with_extension("lock"))
        .map_err(error)?;
    lock.try_lock().map_err(error)?;
    if socket.exists() {
        fs::remove_file(socket).map_err(error)?;
    }
    let listener = UnixListener::bind(socket).map_err(error)?;
    let (tx, rx) = mpsc::sync_channel(128);
    shutdown_on_signal(tx.clone())?;
    let requests = tx.clone();
    thread::spawn(move || {
        for conn in listener.incoming().map_while(Result::ok) {
            if requests.send(Action::Attach(conn)).is_err() {
                break;
            }
        }
    });
    let wake = tx.clone();
    let providers = ProviderRuntime::default();
    let runtime = Runtime::with_providers(providers.clone(), move || {
        let _ = wake.try_send(Action::Wake);
    });
    let result = dispatch(root, repository, providers, runtime, tx, rx);
    let _ = fs::remove_file(socket);
    result
}

fn shutdown_on_signal(tx: mpsc::SyncSender<Action>) -> Result<(), String> {
    use tokio::signal::unix::{signal, SignalKind};
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(error)?;
    let (mut term, mut interrupt) = {
        let _enter = rt.enter();
        (
            signal(SignalKind::terminate()).map_err(error)?,
            signal(SignalKind::interrupt()).map_err(error)?,
        )
    };
    thread::spawn(move || {
        rt.block_on(async {
            tokio::select! { _=term.recv()=>(), _=interrupt.recv()=>() }
        });
        let _ = tx.send(Action::Shutdown);
    });
    Ok(())
}

fn dispatch(
    root: String,
    repository: String,
    providers: ProviderRuntime,
    mut runtime: Runtime,
    tx: mpsc::SyncSender<Action>,
    rx: mpsc::Receiver<Action>,
) -> Result<(), String> {
    let mut sequence = 0u64;
    let mut generation = 0u64;
    let mut pending = HashMap::new();
    let mut client: Option<UnixStream> = None;
    runtime.watch_checkouts(vec![root.clone()]);
    for action in rx {
        match action {
            Action::Shutdown => break,
            Action::Attach(mut conn) => {
                conn.set_write_timeout(Some(Duration::from_secs(5)))
                    .map_err(error)?;
                if client.is_some() {
                    let _ = write_frame(
                        &mut conn,
                        &ServerFrame::Error("This VM workspace is already open.".into()),
                    );
                    continue;
                }
                if write_frame(
                    &mut conn,
                    &ServerFrame::Ready {
                        version: VERSION,
                        root: root.clone(),
                        repository: repository.clone(),
                    },
                )
                .is_err()
                    || write_frame(
                        &mut conn,
                        &ServerFrame::from(Event::Approvals(providers.approvals.pending())),
                    )
                    .is_err()
                    || write_frame(
                        &mut conn,
                        &ServerFrame::from(Event::Questions(providers.questions.pending())),
                    )
                    .is_err()
                {
                    continue;
                }
                generation += 1;
                let owner = generation;
                let input = tx.clone();
                let mut reader = conn.try_clone().map_err(error)?;
                thread::spawn(move || {
                    while let Ok(frame) = read_frame(&mut reader) {
                        if input.send(Action::Request(owner, Box::new(frame))).is_err() {
                            return;
                        }
                    }
                    let _ = input.send(Action::Closed(owner));
                });
                client = Some(conn);
            }
            Action::Request(owner, request) if owner == generation && client.is_some() => {
                let CommandFrame(id, cmd) = *request;
                sequence += 1;
                let result = if !admitted(&cmd, &root) {
                    Err("This operation is not available for this VM checkout.".into())
                } else if pending.len() >= 1024 {
                    Err("The VM workspace has too many pending requests.".into())
                } else {
                    runtime.submit(sequence, cmd)
                };
                match result {
                    Ok(()) => {
                        pending.insert(sequence, id);
                    }
                    Err(e) => deliver(&mut client, Event::Completed(id, Err(e))),
                }
            }
            Action::Closed(owner) if owner == generation => {
                if let Some(c) = client.take() {
                    let _ = c.shutdown(Shutdown::Both);
                }
                pending.clear();
            }
            _ => {}
        }
        for event in runtime.drain() {
            let event = if let Event::Completed(seq, result) = event {
                let Some(id) = pending.remove(&seq) else {
                    continue;
                };
                Event::Completed(id, result)
            } else {
                event
            };
            deliver(&mut client, event);
        }
        if client.is_none() {
            pending.clear();
        }
    }
    Ok(())
}

fn deliver(client: &mut Option<UnixStream>, event: Event) {
    if let Some(c) = client.as_mut() {
        if write_frame(c, &ServerFrame::from(event)).is_err() {
            let _ = c.shutdown(Shutdown::Both);
            *client = None;
        }
    }
}

#[cfg(test)]
mod tests;

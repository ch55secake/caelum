//! One desktop per persisted workspace, with event-driven activation over the session bus.
use std::{path::Path, sync::mpsc, thread, time::Duration};

use rune::AppHandle;
use tokio::sync::oneshot;
use zbus::{Connection, fdo::RequestNameFlags};

use crate::Workspace;

const OBJECT: &str = "/dev/caelum/Desktop";
const INTERFACE: &str = "dev.caelum.Desktop";

pub(super) enum Launch {
    Primary(Instance),
    Forwarded,
}

pub(super) struct Instance {
    stop: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Activation(AppHandle<Workspace>);

#[zbus::interface(name = "dev.caelum.Desktop")]
impl Activation {
    fn activate(&self, project: &str) -> zbus::fdo::Result<()> {
        if project.len() > 64 * 1024
            || project.contains('\0')
            || (!project.is_empty() && !Path::new(project).is_absolute())
        {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Expected an absolute project path".into(),
            ));
        }
        let project = (!project.is_empty()).then(|| project.to_owned());
        self.0.update(move |s, _| super::activate(s, project));
        Ok(())
    }
}

pub(super) fn start_for_state(
    handle: AppHandle<Workspace>,
    project: Option<String>,
    state: &Path,
) -> Result<Launch, String> {
    let name = instance_name(state)?;
    let (ready, result) = mpsc::sync_channel(1);
    let (stop, stopped) = oneshot::channel();
    let worker = thread::Builder::new()
        .name("desktop-activation".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready.send(Err(error.to_string()));
                    return;
                }
            };
            runtime.block_on(async move {
                let connection =
                    tokio::time::timeout(Duration::from_secs(5), connect(handle, &name, project))
                        .await;
                match connection {
                    Ok(Ok(Some(connection))) => {
                        if ready.send(Ok(true)).is_ok() {
                            let _ = stopped.await;
                        }
                        // Dropping the Tokio runtime can abort the connection's
                        // cleanup before the bus observes it. Finish releasing
                        // ownership before an immediate relaunch can forward here.
                        let _ = tokio::time::timeout(
                            Duration::from_secs(5),
                            connection.release_name(name.as_str()),
                        )
                        .await;
                        drop(connection);
                    }
                    Ok(Ok(None)) => {
                        let _ = ready.send(Ok(false));
                    }
                    Ok(Err(error)) => {
                        let _ = ready.send(Err(error.to_string()));
                    }
                    Err(_) => {
                        let _ = ready.send(Err("Desktop activation timed out".into()));
                    }
                }
            });
        })
        .map_err(|error| error.to_string())?;
    match result.recv().map_err(|error| error.to_string())? {
        Ok(true) => Ok(Launch::Primary(Instance {
            stop: Some(stop),
            worker: Some(worker),
        })),
        Ok(false) => {
            let _ = worker.join();
            Ok(Launch::Forwarded)
        }
        Err(error) => {
            let _ = worker.join();
            Err(format!(
                "Cannot activate the Linux desktop: {error}. A D-Bus session is required."
            ))
        }
    }
}

async fn connect(
    handle: AppHandle<Workspace>,
    name: &str,
    project: Option<String>,
) -> zbus::Result<Option<Connection>> {
    let connection = Connection::session().await?;
    connection
        .object_server()
        .at(OBJECT, Activation(handle))
        .await?;
    for attempt in 0..3 {
        match connection
            .request_name_with_flags(name, RequestNameFlags::DO_NOT_QUEUE)
            .await
        {
            Ok(_) => return Ok(Some(connection)),
            Err(zbus::Error::NameTaken) => {
                let proxy = zbus::Proxy::new(&connection, name, OBJECT, INTERFACE).await?;
                match proxy
                    .call::<_, _, ()>("Activate", &(project.as_deref().unwrap_or_default(),))
                    .await
                {
                    Ok(()) => return Ok(None),
                    // The owner can exit between RequestName and Activate. Retry
                    // ownership, bounded by both attempts and the launch timeout.
                    Err(zbus::Error::FDO(error))
                        if attempt < 2
                            && matches!(
                                *error,
                                zbus::fdo::Error::NoReply(_)
                                    | zbus::fdo::Error::NameHasNoOwner(_)
                                    | zbus::fdo::Error::ServiceUnknown(_)
                            ) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the last attempt returns its result")
}

fn instance_name(state: &Path) -> Result<String, String> {
    let state = std::path::absolute(state).map_err(|error| error.to_string())?;
    let parent = state
        .parent()
        .ok_or("The workspace must have a parent directory")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let canonical = state
        .canonicalize()
        .or_else(|_| {
            parent
                .canonicalize()
                .map(|parent| parent.join(state.file_name().unwrap_or_default()))
        })
        .map_err(|error| error.to_string())?;
    let hash = blake3::hash(canonical.as_os_str().as_encoded_bytes());
    Ok(format!("dev.caelum.desktop.s{hash}"))
}

#[cfg(test)]
mod tests;

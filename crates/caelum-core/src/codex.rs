use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};

use caelum_protocol::CodexEvent;
use serde_json::{json, Value};
use thiserror::Error;

mod requests;
mod sessions;
#[cfg(all(test, unix))]
mod tests;
mod transport;

use transport::spawn_reader;

use crate::{
    approvals::ApprovalBroker,
    processes::{ProcessId, ProcessRegistry},
    projects::{Project, ProjectError},
    questions::QuestionBroker,
};

const PROVIDER_NAME: &str = "codex";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

type PendingRequests = Arc<Mutex<HashMap<String, Sender<Result<Value, String>>>>>;
type EventSubscribers = Arc<Mutex<Vec<Sender<CodexEvent>>>>;

/// Manages project-scoped Codex app-server connections and their threads.
/// Each connection uses Codex's local stdio JSONL transport.
#[derive(Clone)]
pub struct CodexProvider {
    inner: Arc<ProviderInner>,
}

struct ProviderInner {
    executable: Option<PathBuf>,
    processes: Arc<ProcessRegistry>,
    approvals: ApprovalBroker,
    questions: QuestionBroker,
    connections: Mutex<HashMap<PathBuf, Arc<CodexConnection>>>,
    subscribers: EventSubscribers,
    closed: AtomicBool,
}

struct CodexConnection {
    process_id: ProcessId,
    writer: Arc<Mutex<std::process::ChildStdin>>,
    pending: PendingRequests,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
    thread_ids: Mutex<HashSet<String>>,
    pending_turns: Arc<Mutex<VecDeque<String>>>,
    turn_threads: Arc<Mutex<HashMap<String, String>>>,
}

struct ReaderContext {
    pending: PendingRequests,
    writer: Arc<Mutex<std::process::ChildStdin>>,
    alive: Arc<AtomicBool>,
    subscribers: EventSubscribers,
    approvals: ApprovalBroker,
    questions: QuestionBroker,
    project_path: PathBuf,
    pending_turns: Arc<Mutex<VecDeque<String>>>,
    turn_threads: Arc<Mutex<HashMap<String, String>>>,
}

impl CodexProvider {
    pub fn new(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        questions: QuestionBroker,
    ) -> Self {
        Self::with_optional_executable(processes, approvals, questions, None)
    }

    /// Construct a provider with an explicit Codex executable. This is useful
    /// for installations outside `PATH` and for protocol-compatible clients.
    pub fn with_executable(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        questions: QuestionBroker,
        executable: impl Into<PathBuf>,
    ) -> Self {
        Self::with_optional_executable(processes, approvals, questions, Some(executable.into()))
    }

    fn with_optional_executable(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        questions: QuestionBroker,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                executable,
                processes,
                approvals,
                questions,
                connections: Mutex::new(HashMap::new()),
                subscribers: Arc::new(Mutex::new(Vec::new())),
                closed: AtomicBool::new(false),
            }),
        }
    }

    /// Subscribe to Codex thread, turn, item, and process events.
    pub fn subscribe(&self) -> Receiver<CodexEvent> {
        let (sender, receiver) = mpsc::channel();
        if !self.inner.closed.load(Ordering::Acquire) {
            self.inner.lock_subscribers().push(sender);
        }
        receiver
    }

    pub fn ensure_checkout_idle(&self, checkout: &Path) -> Result<(), CodexError> {
        let connections: Vec<_> = self
            .inner
            .lock_connections()
            .iter()
            .filter(|(path, connection)| {
                path.starts_with(checkout) && connection.alive.load(Ordering::Acquire)
            })
            .map(|(path, connection)| (path.clone(), Arc::clone(connection)))
            .collect();
        for (path, connection) in connections {
            let starting = !connection
                .pending_turns
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty();
            if starting
                || Self::list_sessions_on(&Project::open(&path)?, &connection, false)?
                    .threads
                    .iter()
                    .any(|thread| thread.is_working)
            {
                return Err(CodexError::Protocol(
                    "Stop running Codex sessions before removing this worktree.".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn release_checkout(&self, checkout: &Path) {
        let mut connections = self.inner.lock_connections();
        let paths: Vec<_> = connections
            .keys()
            .filter(|path| path.starts_with(checkout))
            .cloned()
            .collect();
        for path in paths {
            if let Some(connection) = connections.remove(&path) {
                self.inner
                    .questions
                    .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
                let _ = self.inner.processes.terminate(connection.process_id);
            }
        }
    }

    /// Terminate every app-server process owned by this provider.
    pub fn shutdown(&self) {
        if self.inner.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let connections = {
            let mut connections = self.inner.lock_connections();
            std::mem::take(&mut *connections)
        };
        for (path, connection) in connections {
            self.inner
                .questions
                .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
            let _ = self.inner.processes.terminate(connection.process_id);
        }
        self.inner.lock_subscribers().clear();
    }

    fn connection(&self, project: &Project) -> Result<Arc<CodexConnection>, CodexError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(CodexError::Closed);
        }

        let mut connections = self.inner.lock_connections();
        if let Some(connection) = connections.get(project.path()) {
            if connection.alive.load(Ordering::Acquire) {
                return Ok(Arc::clone(connection));
            }
            let dead = connections
                .remove(project.path())
                .expect("entry was just found");
            let _ = self.inner.processes.terminate(dead.process_id);
        }

        let executable = match self.inner.executable.as_deref() {
            Some(path) if crate::executable::executable(path) => path.to_path_buf(),
            Some(path) => return Err(CodexError::ExecutableNotFound(path.to_path_buf())),
            None => find_codex_executable()?,
        };
        check_authentication(&executable, project.path())?;

        let mut child = Command::new(&executable)
            .env("PATH", crate::executable::path(&executable))
            .args(["app-server", "--listen", "stdio://"])
            .current_dir(project.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| CodexError::StartServer {
                executable: executable.clone(),
                source,
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CodexError::Protocol("Codex app-server stdin was not piped".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CodexError::Protocol("Codex app-server stdout was not piped".into()))?;
        let process_id = self.inner.processes.register(child)?;
        let connection = Arc::new(CodexConnection {
            process_id,
            writer: Arc::new(Mutex::new(stdin)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            alive: Arc::new(AtomicBool::new(true)),
            thread_ids: Mutex::new(HashSet::new()),
            pending_turns: Arc::new(Mutex::new(VecDeque::new())),
            turn_threads: Arc::new(Mutex::new(HashMap::new())),
        });

        spawn_reader(
            stdout,
            ReaderContext {
                pending: Arc::clone(&connection.pending),
                writer: Arc::clone(&connection.writer),
                alive: Arc::clone(&connection.alive),
                subscribers: Arc::clone(&self.inner.subscribers),
                approvals: self.inner.approvals.clone(),
                questions: self.inner.questions.clone(),
                project_path: project.path().to_path_buf(),
                pending_turns: Arc::clone(&connection.pending_turns),
                turn_threads: Arc::clone(&connection.turn_threads),
            },
        );

        let init_result = connection
            .request(
                "initialize",
                json!({
                    "capabilities": { "experimentalApi": true },
                    "clientInfo": {
                        "name": "caelum",
                        "title": "Caelum",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
            .and_then(|_| {
                connection.notify("initialized", json!({}))?;
                Ok(Value::Null)
            });
        if let Err(error) = init_result {
            let _ = self.inner.processes.terminate(process_id);
            connection.alive.store(false, Ordering::Release);
            return Err(error);
        }

        connections.insert(project.path().to_path_buf(), Arc::clone(&connection));
        Ok(connection)
    }

    fn connection_for_thread(
        &self,
        project: &Project,
        thread_id: &str,
    ) -> Result<Arc<CodexConnection>, CodexError> {
        let mut connection = self.connection(project)?;
        if !connection.owns_thread(thread_id) {
            self.list_sessions(project.path())?;
            connection = self.connection(project)?;
        }
        if !connection.owns_thread(thread_id) {
            return Err(CodexError::ThreadNotInProject);
        }
        Ok(connection)
    }
}

impl Drop for ProviderInner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        let connections = self
            .connections
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (path, connection) in std::mem::take(connections) {
            let _ = self.processes.terminate(connection.process_id);
            self.questions
                .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
        }
    }
}

fn id_key(id: &Value) -> String {
    match id {
        Value::String(value) => value.clone(),
        _ => id.to_string(),
    }
}

fn request_id_string(id: &Value) -> Result<String, ()> {
    match id {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(()),
    }
}

fn project_key(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn find_codex_executable() -> Result<PathBuf, CodexError> {
    crate::executable::find("codex").ok_or(CodexError::NotInstalled)
}

fn check_authentication(executable: &Path, project_path: &Path) -> Result<(), CodexError> {
    let output = Command::new(executable)
        .env("PATH", crate::executable::path(executable))
        .args(["login", "status"])
        .current_dir(project_path)
        .output()
        .map_err(|source| CodexError::CheckAuthentication {
            executable: executable.to_path_buf(),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CodexError::NotAuthenticated)
    }
}

#[derive(Debug, Error)]
pub enum CodexError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("Codex CLI was not found. Install Codex CLI, then refresh provider status.")]
    NotInstalled,
    #[error("Codex executable was not found at '{0}'. Check the configured Codex CLI path.")]
    ExecutableNotFound(PathBuf),
    #[error("Codex is not signed in. Run `codex login` in a terminal, then try again.")]
    NotAuthenticated,
    #[error("could not check Codex authentication using '{executable}': {source}")]
    CheckAuthentication {
        executable: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not start Codex app-server using '{executable}': {source}")]
    StartServer {
        executable: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Codex app-server transport failed: {0}")]
    Transport(String),
    #[error("Codex app-server request failed: {0}")]
    Rpc(String),
    #[error("Codex app-server protocol error: {0}")]
    Protocol(String),
    #[error("Codex app-server request '{0}' timed out")]
    RequestTimedOut(String),
    #[error("Codex app-server connection closed unexpectedly")]
    ConnectionClosed,
    #[error("Codex session ID cannot be empty")]
    InvalidThreadId,
    #[error("Codex session does not belong to this project")]
    ThreadNotInProject,
    #[error("Prompt cannot be empty")]
    EmptyPrompt,
    #[error("Codex provider is shutting down")]
    Closed,
    #[error(transparent)]
    Process(#[from] crate::processes::ProcessError),
}

impl ProviderInner {
    fn lock_connections(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<CodexConnection>>> {
        self.connections.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn lock_subscribers(&self) -> MutexGuard<'_, Vec<Sender<CodexEvent>>> {
        self.subscribers.lock().unwrap_or_else(|p| p.into_inner())
    }
}

use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::Duration,
};

use caelum_protocol::{
    ApprovalDecision, CodexEvent, CodexSessionInfo, CodexSessionList, CodexThreadInfo, LineChanges,
    PermissionRequest, ProviderChoice, ProviderChoices, ProviderModelChoice, QuestionPrompt,
    SessionDetails, SessionHistoryPage,
};
use serde_json::{json, Value};
use thiserror::Error;

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

    /// Return recent Codex threads whose working directory is this project.
    pub fn list_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<CodexSessionList, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        Self::list_sessions_on(&project, &connection)
    }

    fn list_sessions_on(
        project: &Project,
        connection: &CodexConnection,
    ) -> Result<CodexSessionList, CodexError> {
        let mut cursor = Value::Null;
        let mut seen_cursors = HashSet::new();
        let mut threads = Vec::new();

        for _ in 0..100 {
            let response = connection.request(
                "thread/list",
                json!({
                    "cursor": cursor,
                    "limit": 100,
                    "sortKey": "updated_at",
                    "archived": false,
                    "cwd": project.path().to_string_lossy(),
                    "sourceKinds": ["appServer", "cli", "vscode"]
                }),
            )?;
            let data = response
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    CodexError::Protocol("thread/list response has no data array".into())
                })?;
            for value in data {
                if let Some(thread) = thread_info(value) {
                    connection.remember_thread(&thread.id);
                    threads.push(thread);
                }
            }

            let Some(next) = response.get("nextCursor").filter(|value| !value.is_null()) else {
                break;
            };
            let Some(next_cursor) = next.as_str() else {
                return Err(CodexError::Protocol(
                    "thread/list returned a non-string cursor".into(),
                ));
            };
            if !seen_cursors.insert(next_cursor.to_owned()) {
                return Err(CodexError::Protocol(
                    "thread/list returned a repeated cursor".into(),
                ));
            }
            cursor = Value::String(next_cursor.to_owned());
        }

        Ok(CodexSessionList {
            project: project.info(),
            threads,
        })
    }

    pub fn start_session(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<CodexSessionInfo, CodexError> {
        self.start_session_with_model(project_path, None)
    }

    pub fn choices(&self, project_path: impl AsRef<Path>) -> Result<ProviderChoices, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        let mut models = Vec::new();
        let mut default_model = None;
        let mut cursor: Option<String> = None;
        let mut seen = HashSet::new();
        loop {
            let response =
                connection.request("model/list", json!({ "limit": 100, "cursor": cursor }))?;
            let data = response
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| CodexError::Protocol("model/list has no data array".into()))?;
            models.extend(data.iter().filter_map(|model| {
                if model.get("isDefault").and_then(Value::as_bool) == Some(true) {
                    default_model = model
                        .get("model")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                }
                Some(ProviderModelChoice {
                    id: model.get("model")?.as_str()?.to_owned(),
                    name: model.get("displayName")?.as_str()?.to_owned(),
                    provider: None,
                    // Codex reports the effective context limit with thread token usage.
                    context_window: None,
                    efforts: model
                        .get("supportedReasoningEfforts")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|effort| {
                            let id = effort.get("reasoningEffort")?.as_str()?.to_owned();
                            Some(ProviderChoice {
                                name: id.clone(),
                                id,
                            })
                        })
                        .collect(),
                    default_effort: model
                        .get("defaultReasoningEffort")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
            }));
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            match &cursor {
                Some(next) if seen.insert(next.clone()) => {}
                Some(_) => return Err(CodexError::Protocol("model/list repeated cursor".into())),
                None => break,
            }
        }
        // Resolve project-specific defaults before offering effort options for
        // the otherwise implicit model in a new session.
        if let Ok(config) = connection.request(
            "config/read",
            json!({ "cwd": project.path().to_string_lossy(), "includeLayers": false }),
        ) {
            if let Some(model) = config.pointer("/config/model").and_then(Value::as_str) {
                default_model = Some(model.to_owned());
            }
        }
        Ok(ProviderChoices {
            models,
            agents: Vec::new(),
            default_model,
        })
    }

    pub fn start_session_with_model(
        &self,
        project_path: impl AsRef<Path>,
        model: Option<&str>,
    ) -> Result<CodexSessionInfo, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        let response = connection.request(
            "thread/start",
            json!({
                "cwd": project.path().to_string_lossy(),
                "approvalPolicy": "unlessTrusted",
                "sandbox": "workspaceWrite",
                "serviceName": "caelum",
                "model": model
            }),
        )?;
        let thread = response
            .get("thread")
            .and_then(thread_info)
            .ok_or_else(|| CodexError::Protocol("thread/start response has no thread".into()))?;
        connection.remember_thread(&thread.id);
        Ok(CodexSessionInfo {
            project: project.info(),
            thread,
            effort: response
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_owned),
            model: response
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    pub fn resume_session(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
    ) -> Result<CodexSessionInfo, CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        let project = Project::open(project_path)?;
        let connection = self.connection_for_thread(&project, thread_id)?;
        let response = connection.request(
            "thread/resume",
            json!({ "threadId": thread_id, "excludeTurns": true }),
        )?;
        let thread = response
            .get("thread")
            .and_then(thread_info)
            .ok_or_else(|| CodexError::Protocol("thread/resume response has no thread".into()))?;
        connection.remember_thread(&thread.id);
        Ok(CodexSessionInfo {
            project: project.info(),
            thread,
            effort: response
                .get("reasoningEffort")
                .and_then(Value::as_str)
                .map(str::to_owned),
            model: response
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    pub fn session_history(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
        cursor: Option<&str>,
    ) -> Result<SessionHistoryPage, CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        let project = Project::open(project_path)?;
        let connection = self.connection_for_thread(&project, thread_id)?;
        let response = connection.request(
            "thread/turns/list",
            json!({ "threadId": thread_id, "limit": 30, "sortDirection": "desc", "itemsView": "full", "cursor": cursor }),
        )?;
        let data = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| CodexError::Protocol("thread/turns/list has no data array".into()))?;
        Ok(SessionHistoryPage {
            agent: None,
            model: None,
            effort: None,
            is_working: cursor.is_none().then(|| {
                data.first().is_some_and(|turn| {
                    turn.get("status").and_then(Value::as_str) == Some("inProgress")
                })
            }),
            data: data.iter().rev().cloned().collect(),
            next_cursor: response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    /// Submit one text prompt. The call returns once Codex accepts the turn;
    /// later output is delivered through `subscribe`.
    pub fn send_prompt(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
        prompt: &str,
    ) -> Result<(), CodexError> {
        self.send_prompt_with_model(project_path, thread_id, prompt, None)
    }

    pub fn send_prompt_with_model(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
        prompt: &str,
        model: Option<&str>,
    ) -> Result<(), CodexError> {
        self.send_prompt_with_options(project_path, thread_id, prompt, model, None)
    }

    pub fn send_prompt_with_options(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
        prompt: &str,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<(), CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        if prompt.trim().is_empty() {
            return Err(CodexError::EmptyPrompt);
        }
        let project = Project::open(project_path)?;
        let connection = self.connection_for_thread(&project, thread_id)?;
        connection.queue_turn(thread_id);
        match connection.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "model": model,
                "effort": effort,
                "input": [{ "type": "text", "text": prompt }]
            }),
        ) {
            Ok(result) => connection.complete_turn_start(thread_id, &result),
            Err(error) => {
                connection.remove_pending_turn(thread_id);
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn session_details(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
    ) -> Result<SessionDetails, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection_for_thread(&project, thread_id)?;
        let response = connection.request("thread/read", json!({ "threadId": thread_id }))?;
        let branch = response
            .pointer("/thread/gitInfo/branch")
            .and_then(Value::as_str)
            .map(str::to_owned);
        // A newly created thread may not have materialized history yet.
        let response = connection.request("thread/turns/list", json!({ "threadId": thread_id, "limit": 1, "sortDirection": "desc", "itemsView": "full" })).ok();
        let changes = response
            .as_ref()
            .and_then(|response| response.pointer("/data/0/items"))
            .and_then(Value::as_array)
            .map(|items| {
                let mut total = LineChanges::default();
                for item in items.iter().filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("fileChange")
                        && item.get("status").and_then(Value::as_str) == Some("completed")
                }) {
                    for change in item
                        .get("changes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let counts = file_change_lines(change);
                        total.additions += counts.additions;
                        total.deletions += counts.deletions;
                    }
                }
                total
            });
        Ok(SessionDetails {
            branch,
            branch_is_current: false,
            changes,
        })
    }

    pub fn set_session_archived(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
        archived: bool,
    ) -> Result<(), CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        let project = Project::open(project_path)?;
        // Read also works for archived threads, which are absent from thread/list.
        let connection = self.connection(&project)?;
        let response = connection.request("thread/read", json!({ "threadId": thread_id }))?;
        let cwd = response
            .pointer("/thread/cwd")
            .and_then(Value::as_str)
            .ok_or(CodexError::ThreadNotInProject)?;
        if Path::new(cwd).canonicalize().ok().as_deref() != Some(project.path()) {
            return Err(CodexError::ThreadNotInProject);
        }
        if archived
            && response
                .pointer("/thread/status/type")
                .and_then(Value::as_str)
                == Some("active")
        {
            return Err(CodexError::Protocol(
                "stop the running turn before archiving this session".into(),
            ));
        }
        connection.request(
            if archived {
                "thread/archive"
            } else {
                "thread/unarchive"
            },
            json!({ "threadId": thread_id }),
        )?;
        if archived {
            self.inner.questions.clear(
                PROVIDER_NAME,
                &project.path().to_string_lossy(),
                Some(thread_id),
            );
        }
        Ok(())
    }

    pub fn cancel_turn(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
    ) -> Result<(), CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        let project = Project::open(project_path)?;
        let connection = self.connection_for_thread(&project, thread_id)?;
        connection.request("turn/interrupt", json!({ "threadId": thread_id }))?;
        self.inner.questions.clear(
            PROVIDER_NAME,
            &project.path().to_string_lossy(),
            Some(thread_id),
        );
        Ok(())
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
                || Self::list_sessions_on(&Project::open(&path)?, &connection)?
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
            Some(path) if path.is_file() => path.to_path_buf(),
            Some(path) => return Err(CodexError::ExecutableNotFound(path.to_path_buf())),
            None => find_codex_executable()?,
        };
        check_authentication(&executable, project.path())?;

        let mut child = Command::new(&executable)
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

impl CodexConnection {
    fn queue_turn(&self, thread_id: &str) {
        self.pending_turns
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(thread_id.to_owned());
    }

    fn complete_turn_start(&self, thread_id: &str, result: &Value) {
        let turn_id = result
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str);
        let mut pending = self.pending_turns.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(index) = pending
            .iter()
            .position(|pending_id| pending_id == thread_id)
        {
            pending.remove(index);
        }
        if let Some(turn_id) = turn_id {
            self.turn_threads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(turn_id.to_owned(), thread_id.to_owned());
        }
    }

    fn remove_pending_turn(&self, thread_id: &str) {
        let mut pending = self.pending_turns.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(index) = pending
            .iter()
            .position(|pending_id| pending_id == thread_id)
        {
            pending.remove(index);
        }
    }

    fn remember_thread(&self, thread_id: &str) {
        self.thread_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(thread_id.to_owned());
    }

    fn owns_thread(&self, thread_id: &str) -> bool {
        self.thread_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(thread_id)
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, CodexError> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(CodexError::ConnectionClosed);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let id_key = id.to_string();
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id_key.clone(), sender);
        if let Err(error) = write_message(
            &self.writer,
            &json!({ "method": method, "id": id, "params": params }),
        ) {
            self.pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id_key);
            return Err(CodexError::Transport(error));
        }

        match receiver.recv_timeout(REQUEST_TIMEOUT) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(message)) => Err(CodexError::Rpc(message)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&id_key);
                Err(CodexError::RequestTimedOut(method.to_owned()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(CodexError::ConnectionClosed),
        }
    }

    fn notify(&self, method: &str, params: Value) -> Result<(), CodexError> {
        write_message(&self.writer, &json!({ "method": method, "params": params }))
            .map_err(CodexError::Transport)
    }
}

fn spawn_reader(stdout: std::process::ChildStdout, context: ReaderContext) {
    thread::spawn(move || {
        let ReaderContext {
            pending,
            writer,
            alive,
            subscribers,
            approvals,
            questions,
            project_path,
            pending_turns,
            turn_threads,
        } = context;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) => {
                    publish_event(
                        &subscribers,
                        codex_event(
                            &project_path,
                            "codex/transportError",
                            json!({ "message": error.to_string() }),
                            None,
                        ),
                    );
                    break;
                }
            }

            let message: Value = match serde_json::from_str(line.trim_end()) {
                Ok(message) => message,
                Err(error) => {
                    publish_event(
                        &subscribers,
                        codex_event(
                            &project_path,
                            "codex/protocolError",
                            json!({ "message": error.to_string() }),
                            None,
                        ),
                    );
                    continue;
                }
            };

            if let Some(id) = message.get("id") {
                if let Some(method) = message.get("method").and_then(Value::as_str) {
                    if method == "item/tool/requestUserInput" {
                        let result = request_question(
                            id,
                            &message["params"],
                            &writer,
                            &alive,
                            &questions,
                            &project_path,
                        );
                        if let Err(error) = result {
                            let _ = write_message(
                                &writer,
                                &json!({ "id": id, "error": { "code": -32602, "message": error } }),
                            );
                        }
                    } else {
                        handle_server_request(
                            method,
                            id.clone(),
                            message.get("params").cloned().unwrap_or(Value::Null),
                            &writer,
                            &approvals,
                            &project_path,
                        );
                    }
                    publish_event(
                        &subscribers,
                        codex_event(
                            &project_path,
                            method,
                            message.get("params").cloned().unwrap_or(Value::Null),
                            Some(id.clone()),
                        ),
                    );
                    continue;
                }
                let key = id_key(id);
                if let Some(sender) = pending
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&key)
                {
                    let response = if let Some(error) = message.get("error") {
                        Err(error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("Codex app-server returned an RPC error")
                            .to_owned())
                    } else if let Some(result) = message.get("result") {
                        Ok(result.clone())
                    } else {
                        Err("Codex app-server response contained neither result nor error".into())
                    };
                    let _ = sender.send(response);
                    continue;
                }

                continue;
            }

            if let Some(method) = message.get("method").and_then(Value::as_str) {
                let mut params = message.get("params").cloned().unwrap_or(Value::Null);
                if method == "serverRequest/resolved" {
                    if let Some(request_id) = params.get("requestId") {
                        if let Some(session_id) = params.get("threadId").and_then(Value::as_str) {
                            questions.dismiss(
                                PROVIDER_NAME,
                                &project_path.to_string_lossy(),
                                session_id,
                                &request_id.to_string(),
                            );
                        }
                        if let Ok(request_id) = request_id_string(request_id) {
                            let opaque_id = format!("{}:{request_id}", project_key(&project_path));
                            let _ = approvals.resolve(
                                PROVIDER_NAME,
                                &project_path.to_string_lossy(),
                                &opaque_id,
                                ApprovalDecision::Deny,
                            );
                        }
                    }
                }
                associate_turn_event(method, &mut params, &pending_turns, &turn_threads);
                if matches!(
                    method,
                    "turn/completed" | "thread/closed" | "thread/archived"
                ) {
                    if let Some(session_id) = params.get("threadId").and_then(Value::as_str) {
                        questions.clear(
                            PROVIDER_NAME,
                            &project_path.to_string_lossy(),
                            Some(session_id),
                        );
                    }
                }
                publish_event(
                    &subscribers,
                    codex_event(&project_path, method, params, None),
                );
            }
        }

        questions.clear(PROVIDER_NAME, &project_path.to_string_lossy(), None);
        alive.store(false, Ordering::Release);
        let failed = std::mem::take(&mut *pending.lock().unwrap_or_else(|p| p.into_inner()));
        for (_, sender) in failed {
            let _ = sender.send(Err("Codex app-server process exited".into()));
        }
        publish_event(
            &subscribers,
            codex_event(&project_path, "codex/processExited", Value::Null, None),
        );
    });
}

fn associate_turn_event(
    method: &str,
    params: &mut Value,
    pending_turns: &Mutex<VecDeque<String>>,
    turn_threads: &Mutex<HashMap<String, String>>,
) {
    if !matches!(method, "turn/started" | "turn/completed") {
        return;
    }
    let Some(params) = params.as_object_mut() else {
        return;
    };
    let turn_id = params
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let event_thread_id = params
        .get("threadId")
        .and_then(Value::as_str)
        .map(str::to_owned);

    let associated_thread_id = event_thread_id.or_else(|| {
        let turn_id = turn_id.as_deref()?;
        if let Some(thread_id) = turn_threads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(turn_id)
            .cloned()
        {
            return Some(thread_id);
        }
        if method == "turn/started" {
            return pending_turns
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .pop_front();
        }
        None
    });

    if let (Some(turn_id), Some(thread_id)) = (turn_id.as_deref(), associated_thread_id.as_deref())
    {
        turn_threads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(turn_id.to_owned(), thread_id.to_owned());
        params.insert("threadId".into(), Value::String(thread_id.to_owned()));
    }
    if method == "turn/completed" {
        if let Some(turn_id) = turn_id {
            turn_threads
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&turn_id);
        }
    }
}

fn codex_event(
    project_path: &Path,
    method: &str,
    params: Value,
    request_id: Option<Value>,
) -> CodexEvent {
    CodexEvent {
        project_path: project_path.to_string_lossy().into_owned(),
        method: method.to_owned(),
        params,
        request_id,
    }
}

fn request_question(
    id: &Value,
    params: &Value,
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    alive: &Arc<AtomicBool>,
    questions: &QuestionBroker,
    project: &Path,
) -> Result<(), String> {
    request_id_string(id).map_err(|_| "Invalid question request ID")?;
    let session_id = params["threadId"]
        .as_str()
        .ok_or("Question has no thread ID")?;
    let values = params["questions"]
        .as_array()
        .ok_or("Question request has no questions")?;
    let fields: Vec<Value> = values.iter().map(|question| {
        let key = question["id"].as_str().ok_or("Question has no ID")?;
        let text = question["question"].as_str().ok_or("Question has no text")?;
        let options: Vec<Value> = question["options"].as_array().into_iter().flatten().map(|option| {
            let label = option["label"].as_str().ok_or("Question option has no label")?;
            Ok(json!({ "value": label, "label": label, "description": option["description"].as_str().unwrap_or("") }))
        }).collect::<Result<_, String>>()?;
        Ok(json!({
            "key": key, "type": "string", "title": question["header"].as_str().unwrap_or("Question"),
            "description": text, "required": true,
            "custom": options.is_empty() || question["isOther"].as_bool().unwrap_or(false),
            "secret": question["isSecret"].as_bool().unwrap_or(false), "options": options,
        }))
    }).collect::<Result<_, String>>()?;
    let prompt = QuestionPrompt {
        provider: PROVIDER_NAME.into(),
        project_path: project.to_string_lossy().into_owned(),
        session_id: session_id.into(),
        request_id: id.to_string(),
        title: "Questions".into(),
        fields: fields.clone(),
    };
    let id = id.clone();
    let writer = Arc::clone(writer);
    let alive = Arc::clone(alive);
    questions.insert(
        prompt,
        Arc::new(move |answer| {
            if !alive.load(Ordering::Acquire) {
                return Err("Codex disconnected before the answer could be sent".into());
            }
            let mut answers = serde_json::Map::new();
            if let Some(answer) = answer {
                if answer
                    .keys()
                    .any(|key| !fields.iter().any(|field| field["key"] == *key))
                {
                    return Err("Answer contains an unknown question".into());
                }
                for field in &fields {
                    let key = field["key"].as_str().expect("validated question key");
                    let text = answer
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .ok_or_else(|| {
                            format!(
                                "Answer {} before submitting",
                                field["title"].as_str().unwrap_or(key)
                            )
                        })?;
                    if field["custom"] != true
                        && !field["options"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|option| option["value"] == text)
                    {
                        return Err("Choose one of the available answers".into());
                    }
                    answers.insert(key.into(), json!({ "answers": [text] }));
                }
            }
            write_message(
                &writer,
                &json!({ "id": id, "result": { "answers": answers } }),
            )
        }),
    )
}

fn handle_server_request(
    method: &str,
    request_id: Value,
    params: Value,
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    approvals: &ApprovalBroker,
    project_path: &Path,
) {
    if !matches!(
        method,
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
    ) {
        let _ = write_message(
            writer,
            &json!({
                "id": request_id,
                "error": { "code": -32601, "message": "Unsupported Codex app-server request" }
            }),
        );
        return;
    }

    let Ok(request_key) = request_id_string(&request_id) else {
        let _ = write_message(
            writer,
            &json!({
                "id": request_id,
                "error": { "code": -32600, "message": "Codex approval request ID must be a string or number" }
            }),
        );
        return;
    };
    let opaque_id = format!("{}:{request_key}", project_key(project_path));
    let command = method == "item/commandExecution/requestApproval";
    let reason = params.get("reason").and_then(Value::as_str).unwrap_or("");
    let command_text = command_description(params.get("command"));
    let cwd = params.get("cwd").and_then(Value::as_str).unwrap_or("");
    let summary = if command {
        "Codex requests approval to run a command"
    } else {
        "Codex requests approval to change files"
    };
    let mut details = Vec::new();
    if !reason.trim().is_empty() {
        details.push(reason.trim().to_owned());
    }
    if command && !command_text.is_empty() {
        details.push(format!("Command: {command_text}"));
    }
    if !cwd.trim().is_empty() {
        details.push(format!("Working directory: {cwd}"));
    }
    if let Some(grant_root) = params.get("grantRoot").and_then(Value::as_str) {
        details.push(format!("Requested file access root: {grant_root}"));
    }
    if let Some(host) = params
        .get("networkApprovalContext")
        .and_then(|context| context.get("host"))
        .and_then(Value::as_str)
    {
        details.push(format!("Requested network destination: {host}"));
    }
    if details.is_empty() {
        details.push("Codex needs your permission to continue this operation.".into());
    }

    let request = PermissionRequest {
        provider: PROVIDER_NAME.to_owned(),
        project_path: project_path.to_string_lossy().into_owned(),
        session_id: params
            .get("threadId")
            .and_then(Value::as_str)
            .or_else(|| {
                params
                    .get("thread")
                    .and_then(|thread| thread.get("id"))
                    .and_then(Value::as_str)
            })
            .map(str::to_owned),
        request_id: opaque_id,
        summary: summary.to_owned(),
        description: details.join("\n"),
        provider_details: json!({ "method": method, "params": params }),
    };
    let waiter = match approvals.request(request.clone()) {
        Ok(waiter) => waiter,
        Err(_) => {
            let _ = respond_to_approval(writer, request_id, "decline");
            return;
        }
    };
    let writer = Arc::clone(writer);
    let available = request
        .provider_details
        .get("params")
        .and_then(|value| value.get("availableDecisions"))
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .filter_map(Value::as_str)
                .any(|option| option == "accept")
        })
        .unwrap_or(true);
    thread::spawn(move || {
        let decision = waiter.wait(APPROVAL_TIMEOUT);
        let response_decision = if decision == ApprovalDecision::AllowOnce && available {
            "accept"
        } else {
            "decline"
        };
        let _ = respond_to_approval(&writer, request_id, response_decision);
    });
}

fn respond_to_approval(
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    request_id: Value,
    decision: &str,
) -> Result<(), String> {
    write_message(
        writer,
        &json!({ "id": request_id, "result": { "decision": decision } }),
    )
}

fn command_description(command: Option<&Value>) -> String {
    match command {
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::String(command)) => command.clone(),
        _ => String::new(),
    }
}

fn write_message(
    writer: &Arc<Mutex<std::process::ChildStdin>>,
    message: &Value,
) -> Result<(), String> {
    let mut writer = writer.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::to_writer(&mut *writer, message).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn publish_event(subscribers: &EventSubscribers, event: CodexEvent) {
    subscribers
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|subscriber| subscriber.send(event.clone()).is_ok());
}

fn file_change_lines(change: &Value) -> LineChanges {
    let diff = change.get("diff").and_then(Value::as_str).unwrap_or("");
    let mut counts = LineChanges::default();
    let mut in_hunk = false;
    let mut unified = false;
    for line in diff.lines() {
        if line.starts_with("@@ ") {
            in_hunk = true;
            unified = true;
        } else if line.starts_with("diff ") {
            in_hunk = false;
        } else if in_hunk {
            if line.starts_with('+') {
                counts.additions += 1;
            }
            if line.starts_with('-') {
                counts.deletions += 1;
            }
        }
    }
    if !unified {
        // Added/deleted files can carry their complete contents rather than a patch.
        match change.pointer("/kind/type").and_then(Value::as_str) {
            Some("add") => counts.additions = diff.lines().count() as u64,
            Some("delete") => counts.deletions = diff.lines().count() as u64,
            _ => {}
        }
    }
    counts
}

fn thread_info(value: &Value) -> Option<CodexThreadInfo> {
    Some(CodexThreadInfo {
        is_working: value.pointer("/status/type").and_then(Value::as_str) == Some("active"),
        id: value.get("id")?.as_str()?.to_owned(),
        name: value.get("name").and_then(Value::as_str).map(str::to_owned),
        preview: value
            .get("preview")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        updated_at: value.get("updatedAt").and_then(Value::as_u64),
    })
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
    let executable_name = if cfg!(windows) { "codex.exe" } else { "codex" };
    if let Some(paths) = env::var_os("PATH") {
        for directory in env::split_paths(&paths) {
            let candidate = directory.join(executable_name);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }

    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        for relative in [
            ".local/bin/codex",
            ".npm-global/bin/codex",
            ".volta/bin/codex",
            ".bun/bin/codex",
        ] {
            let candidate = home.join(relative);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }
    for candidate in ["/opt/homebrew/bin/codex", "/usr/local/bin/codex"] {
        let candidate = PathBuf::from(candidate);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }

    Err(CodexError::NotInstalled)
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn check_authentication(executable: &Path, project_path: &Path) -> Result<(), CodexError> {
    let output = Command::new(executable)
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
    #[error("Codex CLI was not found. Install Codex CLI and ensure `codex` is on PATH, then restart Caelum.")]
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let path = env::temp_dir().join(format!(
                    "caelum-codex-test-{}-{}",
                    std::process::id(),
                    NEXT_DIR.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create test directory: {error}"),
                }
            }
        }

        fn project(&self) -> PathBuf {
            let project = self.0.join("project");
            fs::create_dir_all(&project).expect("create project directory");
            project
        }

        fn fake_codex(&self, authenticated: bool) -> PathBuf {
            let executable = self.0.join("fake-codex");
            let script = format!(
                "#!/bin/sh\nif [ \"$1 $2\" = \"login status\" ]; then exit {}; fi\nexec python3 -c '{}'\n",
                if authenticated { 0 } else { 1 },
                PYTHON_FAKE_SERVER.replace('\'', "'\\''")
            );
            crate::test_support::write_executable(&executable, script);
            executable
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const PYTHON_FAKE_SERVER: &str = r#"
import json, os, sys
models = {'thread-existing': 'saved-model'}
efforts = {'thread-existing': 'low'}
archived = set()
for raw in sys.stdin:
    request = json.loads(raw)
    method = request.get('method')
    request_id = request.get('id')
    params = request.get('params', {})
    if method is None and request_id == 'server-approval-1':
        print(json.dumps({'method': 'test/approvalResolved', 'params': {'decision': request['result']['decision']}}), flush=True)
        continue
    if method is None and request_id in [41, '41']:
        print(json.dumps({'method': 'test/questionResolved', 'params': request}), flush=True)
        continue
    if request_id is None:
        continue
    if method == 'initialize':
        assert params['capabilities']['experimentalApi'] is True
        result = {'userAgent': 'fake-codex'}
    elif method == 'thread/list':
        assert params['archived'] is False
        result = {'data': [] if 'thread-existing' in archived else [{'id': 'thread-existing', 'name': 'Existing', 'preview': 'Earlier work', 'updatedAt': 123, 'status': {'type': 'active' if os.path.exists('active-thread') else 'idle'}}], 'nextCursor': None}
    elif method == 'thread/read':
        result = {'thread': {'id': params['threadId'], 'cwd': os.path.dirname(os.getcwd()) if params['threadId'] == 'foreign-thread' else os.getcwd(), 'gitInfo': {'branch': 'feature/sidebar'}, 'status': {'type': 'active' if os.path.exists('active-thread') else 'idle'}}}
    elif method in ['thread/archive', 'thread/unarchive']:
        if os.path.exists('archive-error'):
            print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'archive unavailable'}}), flush=True)
            continue
        if method == 'thread/archive': archived.add(params['threadId'])
        else: archived.discard(params['threadId'])
        result = {}
    elif method == 'thread/start':
        models['thread-new'] = params.get('model') or 'default-model'
        efforts['thread-new'] = 'medium'
        result = {'thread': {'id': 'thread-new', 'preview': '', 'name': None}, 'model': models['thread-new'], 'reasoningEffort': efforts['thread-new']}
    elif method == 'thread/resume':
        result = {'thread': {'id': params['threadId'], 'name': 'Resumed', 'preview': 'History'}, 'model': models[params['threadId']], 'reasoningEffort': efforts[params['threadId']]}
    elif method == 'model/list':
        result = {'data': [{'model': 'test-model', 'displayName': 'Test model', 'isDefault': True, 'defaultReasoningEffort': 'medium', 'supportedReasoningEfforts': [{'reasoningEffort': 'low'}, {'reasoningEffort': 'medium'}, {'reasoningEffort': 'high'}]}], 'nextCursor': None}
    elif method == 'config/read':
        assert params['cwd'] == os.getcwd()
        result = {'config': {'model': 'configured-model'}}
    elif method == 'thread/turns/list':
        assert params['itemsView'] == 'full' and params['sortDirection'] == 'desc'
        if params['limit'] == 1:
            if os.path.exists('missing-details'):
                print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'no history yet'}}), flush=True)
                continue
            result = {'data': [{'id': 'latest', 'items': [
                {'type': 'fileChange', 'status': 'completed', 'changes': [
                    {'kind': {'type': 'update'}, 'diff': '--- a/file\n+++ b/file\n@@ -1 +1,2 @@\n-old\n+new\n+extra\n'},
                    {'kind': {'type': 'add'}, 'diff': 'first\nsecond\n'}]},
                {'type': 'fileChange', 'status': 'failed', 'changes': [{'kind': {'type': 'add'}, 'diff': 'not applied'}]}]}], 'nextCursor': 'older'}
        elif params.get('cursor'):
            assert params['cursor'] == 'older-cursor'
            result = {'data': [{'id': 'turn-old', 'items': [{'id': 'old-user', 'type': 'userMessage', 'content': [{'type': 'text', 'text': 'Oldest'}]}]}], 'nextCursor': None}
        else:
            assert params['limit'] == 30
            result = {'data': [{'id': 'turn-b', 'items': [{'id': 'cmd', 'type': 'commandExecution', 'command': 'git status', 'aggregatedOutput': 'clean', 'status': 'completed'}]}, {'id': 'turn-a', 'items': []}], 'nextCursor': 'older-cursor'}
    elif method == 'turn/start':
        prompt = params['input'][0]['text']
        if prompt == 'rpc-error':
            print(json.dumps({'id': request_id, 'error': {'code': -32000, 'message': 'fake request error'}}), flush=True)
            continue
        if params.get('model'):
            models[params['threadId']] = params['model']
        if params.get('effort'):
            efforts[params['threadId']] = params['effort']
        if prompt == 'request-approval':
            print(json.dumps({'id': 'server-approval-1', 'method': 'item/commandExecution/requestApproval', 'params': {'threadId': params['threadId'], 'turnId': 'turn-approval', 'itemId': 'command-1', 'command': ['git', 'status'], 'cwd': '/project', 'availableDecisions': ['accept', 'decline']}}), flush=True)
        if prompt == 'request-question':
            for question_id in [41, '41']:
                print(json.dumps({'id': question_id, 'method': 'item/tool/requestUserInput', 'params': {'threadId': params['threadId'], 'turnId': 'turn-question', 'itemId': 'question-tool', 'questions': [
                    {'id': 'choice', 'header': 'Approach', 'question': 'Which approach?', 'options': [{'label': 'Small', 'description': 'Focused change'}, {'label': 'Large', 'description': 'Full rewrite'}]},
                    {'id': 'detail', 'header': 'Detail', 'question': 'Any details?', 'isOther': True, 'isSecret': True, 'options': None}
                ]}}), flush=True)
            print(json.dumps({'id': request_id, 'result': {'turn': {'id': 'turn-question', 'status': 'inProgress'}}}), flush=True)
            continue
        if prompt == 'resolve-question':
            for question_id in [41, '41']:
                print(json.dumps({'method': 'serverRequest/resolved', 'params': {'threadId': params['threadId'], 'requestId': question_id}}), flush=True)
        print(json.dumps({'method': 'turn/started', 'params': {'turn': {'id': 'turn-1'}}}), flush=True)
        print(json.dumps({'method': 'item/agentMessage/delta', 'params': {'threadId': params['threadId'], 'itemId': 'item-1', 'delta': 'Hello from Codex'}}), flush=True)
        print(json.dumps({'method': 'turn/completed', 'params': {'turn': {'id': 'turn-1', 'status': 'completed'}}}), flush=True)
        result = {'turn': {'id': 'turn-1', 'status': 'inProgress'}}
    elif method == 'turn/interrupt':
        result = {}
    else:
        print(json.dumps({'id': request_id, 'error': {'code': -32601, 'message': 'Unknown method'}}), flush=True)
        continue
    print(json.dumps({'id': request_id, 'result': result}), flush=True)
    if method == 'thread/list' and os.path.exists('die-after-list'):
        os.unlink('die-after-list')
        sys.exit(0)
"#;

    #[test]
    fn session_archiving_is_reversible_scoped_and_keeps_recorded_branch_details() {
        let directory = TestDirectory::new();
        let project_path = directory.project();
        let provider = CodexProvider::with_executable(
            Arc::new(ProcessRegistry::default()),
            ApprovalBroker::default(),
            QuestionBroker::default(),
            directory.fake_codex(true),
        );
        let details = provider
            .session_details(&project_path, "thread-existing")
            .unwrap();
        assert_eq!(details.branch.as_deref(), Some("feature/sidebar"));
        assert!(!details.branch_is_current);
        assert_eq!(
            details.changes,
            Some(LineChanges {
                additions: 4,
                deletions: 1
            })
        );
        fs::write(project_path.join("missing-details"), "").unwrap();
        let details = provider
            .session_details(&project_path, "thread-existing")
            .unwrap();
        assert_eq!(details.branch.as_deref(), Some("feature/sidebar"));
        assert_eq!(details.changes, None);
        assert!(matches!(
            provider.session_details(&project_path, "foreign-thread"),
            Err(CodexError::ThreadNotInProject)
        ));
        for archived in [true, false] {
            assert!(matches!(
                provider.set_session_archived(&project_path, "foreign-thread", archived),
                Err(CodexError::ThreadNotInProject)
            ));
        }
        fs::write(project_path.join("active-thread"), "").unwrap();
        assert!(provider
            .set_session_archived(&project_path, "thread-existing", true)
            .is_err());
        assert_eq!(
            provider.list_sessions(&project_path).unwrap().threads.len(),
            1
        );
        fs::remove_file(project_path.join("active-thread")).unwrap();
        fs::write(project_path.join("archive-error"), "").unwrap();
        assert!(matches!(
            provider.set_session_archived(&project_path, "thread-existing", true),
            Err(CodexError::Rpc(_))
        ));
        fs::remove_file(project_path.join("archive-error")).unwrap();
        provider
            .set_session_archived(&project_path, "thread-existing", true)
            .unwrap();
        assert!(provider
            .list_sessions(&project_path)
            .unwrap()
            .threads
            .is_empty());
        provider
            .set_session_archived(&project_path, "thread-existing", false)
            .unwrap();
        assert_eq!(
            provider.list_sessions(&project_path).unwrap().threads[0].id,
            "thread-existing"
        );
        provider.shutdown();
    }

    #[test]
    fn checkout_cleanup_rejects_active_turns_and_releases_only_its_connections() {
        let directory = TestDirectory::new();
        let project = Project::open(directory.project()).unwrap();
        let nested = project.path().join("src");
        let other = directory.0.join("project-other");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&other).unwrap();
        let processes = Arc::new(ProcessRegistry::default());
        let provider = CodexProvider::with_executable(
            Arc::clone(&processes),
            ApprovalBroker::default(),
            QuestionBroker::default(),
            directory.fake_codex(true),
        );
        for path in [project.path(), &nested, &other] {
            provider.list_sessions(path).unwrap();
        }
        fs::write(nested.join("active-thread"), "").unwrap();
        assert!(provider.ensure_checkout_idle(project.path()).is_err());
        fs::remove_file(nested.join("active-thread")).unwrap();
        let connection = provider.connection(&project).unwrap();
        connection.queue_turn("thread-existing");
        assert!(provider.ensure_checkout_idle(project.path()).is_err());
        connection.remove_pending_turn("thread-existing");
        provider.ensure_checkout_idle(project.path()).unwrap();
        let nested_connection = Arc::clone(provider.inner.lock_connections().get(&nested).unwrap());
        nested_connection.alive.store(false, Ordering::Release);
        provider.ensure_checkout_idle(project.path()).unwrap();
        assert_eq!(
            provider
                .inner
                .lock_connections()
                .get(&nested)
                .unwrap()
                .process_id,
            nested_connection.process_id
        );
        provider.release_checkout(project.path());
        assert_eq!(provider.inner.lock_connections().len(), 1);
        for connection in [connection, nested_connection] {
            assert!(matches!(
                processes.terminate(connection.process_id),
                Err(crate::processes::ProcessError::UnknownProcess)
            ));
        }
        provider.list_sessions(&other).unwrap();
        provider.shutdown();
    }

    #[test]
    fn line_counts_handle_patch_headers_context_and_full_file_contents() {
        for (kind, diff, additions, deletions) in [
            ("update", "--- a/file\n+++ b/file\n@@ -1,2 +1,3 @@\n context\n-old\n+++literal\n+\n\\ No newline at end of file\n@@ -9 +10 @@\n-old2\n+new2\n", 3, 2),
            ("add", "first\n\nlast", 3, 0),
            ("delete", "first\nlast\n", 0, 2),
            ("add", "", 0, 0),
            ("update", "Binary files a/file and b/file differ", 0, 0),
        ] {
            assert_eq!(file_change_lines(&json!({ "kind": { "type": kind }, "diff": diff })), LineChanges { additions, deletions });
        }
    }

    #[test]
    fn project_sessions_can_be_listed_started_resumed_and_cancelled() {
        let directory = TestDirectory::new();
        let project_path = directory.project();
        let processes = Arc::new(ProcessRegistry::default());
        let provider = CodexProvider::with_executable(
            Arc::clone(&processes),
            ApprovalBroker::default(),
            QuestionBroker::default(),
            directory.fake_codex(true),
        );

        let sessions = provider.list_sessions(&project_path).unwrap();
        assert_eq!(
            sessions.project.path,
            project_path.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(sessions.threads[0].id, "thread-existing");
        assert_eq!(sessions.threads[0].updated_at, Some(123));
        assert!(matches!(
            provider.send_prompt(&project_path, "another-project-thread", "Do work"),
            Err(CodexError::ThreadNotInProject)
        ));

        let created = provider.start_session(&project_path).unwrap();
        assert_eq!(created.thread.id, "thread-new");
        assert_eq!(created.model.as_deref(), Some("default-model"));
        assert_eq!(created.effort.as_deref(), Some("medium"));
        let resumed = provider
            .resume_session(&project_path, "thread-existing")
            .unwrap();
        assert_eq!(resumed.thread.name.as_deref(), Some("Resumed"));
        assert_eq!(resumed.model.as_deref(), Some("saved-model"));
        assert_eq!(resumed.effort.as_deref(), Some("low"));
        let choices = provider.choices(&project_path).unwrap();
        assert_eq!(choices.models[0].id, "test-model");
        assert_eq!(
            choices.models[0]
                .efforts
                .iter()
                .map(|choice| choice.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "medium", "high"]
        );
        assert_eq!(choices.models[0].default_effort.as_deref(), Some("medium"));
        assert_eq!(choices.default_model.as_deref(), Some("configured-model"));
        let history = provider
            .session_history(&project_path, "thread-existing", None)
            .unwrap();
        assert_eq!(history.data[0]["id"], "turn-a");
        assert_eq!(history.data[1]["items"][0]["aggregatedOutput"], "clean");
        let older = provider
            .session_history(
                &project_path,
                "thread-existing",
                history.next_cursor.as_deref(),
            )
            .unwrap();
        assert_eq!(older.data[0]["items"][0]["content"][0]["text"], "Oldest");
        assert_eq!(older.next_cursor, None);
        assert!(matches!(
            provider.session_history(&project_path, "another-project-thread", None),
            Err(CodexError::ThreadNotInProject)
        ));

        let events = provider.subscribe();
        provider
            .send_prompt(&project_path, "thread-existing", "Say hello")
            .unwrap();
        let streamed = events.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(streamed.method, "turn/started");
        let streamed = events.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(streamed.method, "item/agentMessage/delta");
        assert_eq!(streamed.params["delta"], "Hello from Codex");
        let completed = events.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(completed.method, "turn/completed");
        assert_eq!(completed.params["threadId"], "thread-existing");

        provider
            .cancel_turn(&project_path, "thread-existing")
            .unwrap();
        provider
            .send_prompt_with_options(
                &project_path,
                "thread-existing",
                "Use the selected model",
                Some("test-model"),
                Some("high"),
            )
            .unwrap();
        let resumed = provider
            .resume_session(&project_path, "thread-existing")
            .unwrap();
        assert_eq!(resumed.thread.id, "thread-existing");
        assert_eq!(resumed.model.as_deref(), Some("test-model"));
        assert_eq!(resumed.effort.as_deref(), Some("high"));
        provider
            .send_prompt(&project_path, "thread-existing", "Keep this model")
            .unwrap();
        assert_eq!(
            provider
                .resume_session(&project_path, "thread-existing")
                .unwrap()
                .model
                .as_deref(),
            Some("test-model")
        );
        assert_eq!(
            provider
                .resume_session(&project_path, "thread-existing")
                .unwrap()
                .effort
                .as_deref(),
            Some("high")
        );
        provider
            .send_prompt_with_options(
                &project_path,
                "thread-existing",
                "Change only effort",
                None,
                Some("low"),
            )
            .unwrap();
        let resumed = provider
            .resume_session(&project_path, "thread-existing")
            .unwrap();
        assert_eq!(resumed.model.as_deref(), Some("test-model"));
        assert_eq!(resumed.effort.as_deref(), Some("low"));
        assert!(matches!(
            provider.send_prompt(&project_path, "thread-existing", "rpc-error"),
            Err(CodexError::Rpc(message)) if message == "fake request error"
        ));
        provider.shutdown();
        assert!(processes.poll_exited().unwrap().is_empty());
    }

    #[test]
    fn a_crashed_app_server_is_reaped_and_restarted_for_the_project() {
        let directory = TestDirectory::new();
        let project_path = directory.project();
        fs::write(project_path.join("die-after-list"), "").unwrap();
        let provider = CodexProvider::with_executable(
            Arc::new(ProcessRegistry::default()),
            ApprovalBroker::default(),
            QuestionBroker::default(),
            directory.fake_codex(true),
        );
        let events = provider.subscribe();

        provider.list_sessions(&project_path).unwrap();
        loop {
            let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
            if event.method == "codex/processExited" {
                break;
            }
        }

        let session = provider.start_session(&project_path).unwrap();
        assert_eq!(session.thread.id, "thread-new");
        provider.shutdown();
    }

    #[test]
    fn command_approval_is_forwarded_and_resolved_over_jsonl() {
        let directory = TestDirectory::new();
        let project_path = directory.project();
        let approvals = ApprovalBroker::default();
        let updates = approvals.subscribe();
        let _initial_snapshot = updates.recv_timeout(Duration::from_secs(1)).unwrap();
        let provider = CodexProvider::with_executable(
            Arc::new(ProcessRegistry::default()),
            approvals.clone(),
            QuestionBroker::default(),
            directory.fake_codex(true),
        );
        provider.list_sessions(&project_path).unwrap();
        let events = provider.subscribe();

        provider
            .send_prompt(&project_path, "thread-existing", "request-approval")
            .unwrap();
        let prompts = updates.recv_timeout(Duration::from_secs(2)).unwrap();
        let prompt = prompts.first().expect("Codex approval reaches the broker");
        assert_eq!(prompt.provider, PROVIDER_NAME);
        assert_eq!(
            prompt.project_path,
            project_path.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(prompt.session_id.as_deref(), Some("thread-existing"));
        assert!(prompt.description.contains("git status"));
        approvals
            .resolve(
                PROVIDER_NAME,
                &prompt.project_path,
                &prompt.request_id,
                ApprovalDecision::AllowOnce,
            )
            .unwrap();

        let resolved = loop {
            let event = events.recv_timeout(Duration::from_secs(2)).unwrap();
            if event.method == "test/approvalResolved" {
                break event;
            }
        };
        assert_eq!(resolved.params["decision"], "accept");
        provider.shutdown();
    }

    #[test]
    fn missing_installation_and_authentication_have_actionable_errors() {
        let directory = TestDirectory::new();
        let project_path = directory.project();
        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();

        let missing = CodexProvider::with_executable(
            Arc::clone(&processes),
            approvals.clone(),
            QuestionBroker::default(),
            directory.0.join("missing-codex"),
        );
        assert!(matches!(
            missing.list_sessions(&project_path),
            Err(CodexError::ExecutableNotFound(_))
        ));

        let signed_out = CodexProvider::with_executable(
            processes,
            approvals,
            QuestionBroker::default(),
            directory.fake_codex(false),
        );
        assert!(matches!(
            signed_out.list_sessions(&project_path),
            Err(CodexError::NotAuthenticated)
        ));
    }

    #[test]
    fn questions_round_trip_original_rpc_ids_and_clear_on_resolution_or_interrupt() {
        let directory = TestDirectory::new();
        let project = directory.project();
        let questions = QuestionBroker::default();
        let provider = CodexProvider::with_executable(
            Arc::new(ProcessRegistry::default()),
            ApprovalBroker::default(),
            questions.clone(),
            directory.fake_codex(true),
        );
        provider.list_sessions(&project).unwrap();
        let events = provider.subscribe();
        let next = |method: &str| loop {
            let event = events.recv_timeout(Duration::from_secs(3)).unwrap();
            if event.method == method {
                break event;
            }
        };
        provider
            .send_prompt(&project, "thread-existing", "request-question")
            .unwrap();
        assert_eq!(questions.pending().len(), 2);
        let path = project
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let answer = json!({"choice": "Small", "detail": "Keep compatibility"})
            .as_object()
            .unwrap()
            .clone();
        assert!(questions
            .resolve(
                PROVIDER_NAME,
                &path,
                "thread-existing",
                "41",
                Some(serde_json::Map::new())
            )
            .is_err());
        assert_eq!(questions.pending().len(), 2);
        assert_eq!(questions.pending()[0].fields[1]["secret"], true);
        questions
            .resolve(PROVIDER_NAME, &path, "thread-existing", "41", Some(answer))
            .unwrap();
        assert_eq!(
            next("test/questionResolved").params,
            json!({ "id": 41, "result": {"answers": {
                "choice": {"answers": ["Small"]}, "detail": {"answers": ["Keep compatibility"]}
            }}})
        );
        questions
            .resolve(PROVIDER_NAME, &path, "thread-existing", "\"41\"", None)
            .unwrap();
        assert_eq!(
            next("test/questionResolved").params,
            json!({ "id": "41", "result": { "answers": {} }})
        );
        provider
            .send_prompt(&project, "thread-existing", "request-question")
            .unwrap();
        provider
            .send_prompt(&project, "thread-existing", "resolve-question")
            .unwrap();
        assert!(questions.pending().is_empty());
        provider
            .send_prompt(&project, "thread-existing", "request-question")
            .unwrap();
        provider.cancel_turn(&project, "thread-existing").unwrap();
        assert!(questions.pending().is_empty());
        provider
            .send_prompt(&project, "thread-existing", "request-question")
            .unwrap();
        provider.shutdown();
        assert!(questions.pending().is_empty());
    }
}

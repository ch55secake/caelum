use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{self, BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};

use caelum_protocol::{
    ApprovalDecision, LineChanges, OpenCodeCommandInfo, OpenCodeCommandInput, OpenCodeEvent,
    OpenCodeSession, OpenCodeSessionInfo, OpenCodeSessionList, PermissionRequest, ProviderChoice,
    ProviderChoices, ProviderModelChoice, QuestionPrompt, SessionDetails, SessionHistoryPage,
};
use reqwest::{blocking::Client, Method, StatusCode, Url};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    approvals::ApprovalBroker,
    processes::{ProcessId, ProcessRegistry},
    projects::{Project, ProjectError},
    questions::{QuestionBroker, Reply},
};

const PROVIDER_NAME: &str = "opencode";
const SERVER_PASSWORD_ENV: &str = "OPENCODE_SERVER_PASSWORD";
const SERVER_START_TIMEOUT: Duration = Duration::from_secs(12);
const SERVER_POLL_INTERVAL: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SESSION_LIMIT: &str = "100";

type EventSubscribers = Arc<Mutex<Vec<Sender<OpenCodeEvent>>>>;

/// Connects projects to the registered local OpenCode service, falling back to
/// an owned loopback server. Both use the official `/api` and `/api/event` routes.
#[derive(Clone)]
pub struct OpenCodeProvider {
    inner: Arc<ProviderInner>,
}

struct ProviderInner {
    executable: Option<PathBuf>,
    service_file: Option<PathBuf>,
    processes: Arc<ProcessRegistry>,
    approvals: ApprovalBroker,
    questions: QuestionBroker,
    servers: Mutex<HashMap<PathBuf, Arc<OpenCodeServer>>>,
    subscribers: EventSubscribers,
    closed: AtomicBool,
}

struct OpenCodeServer {
    process_id: Option<ProcessId>,
    base_url: String,
    client: Client,
    event_client: Client,
    password: Option<String>,
    alive: Arc<AtomicBool>,
    session_ids: Mutex<HashSet<String>>,
    pending_permission_ids: Arc<Mutex<HashSet<String>>>,
}

impl OpenCodeProvider {
    pub fn new(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        questions: QuestionBroker,
    ) -> Self {
        Self::with_optional_executable(processes, approvals, questions, None)
    }

    /// Construct a provider with an explicit OpenCode executable. This is
    /// useful for non-standard installations and lifecycle tests.
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
                service_file: executable.is_none().then(service_file).flatten(),
                executable,
                processes,
                approvals,
                questions,
                servers: Mutex::new(HashMap::new()),
                subscribers: Arc::new(Mutex::new(Vec::new())),
                closed: AtomicBool::new(false),
            }),
        }
    }

    /// Subscribe to session, tool, permission, and server lifecycle events.
    pub fn subscribe(&self) -> Receiver<OpenCodeEvent> {
        let (sender, receiver) = mpsc::channel();
        if !self.inner.closed.load(Ordering::Acquire) {
            self.inner.lock_subscribers().push(sender);
        }
        receiver
    }

    pub fn list_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        Self::list_sessions_on(&project, &server)
    }

    fn list_sessions_on(
        project: &Project,
        server: &OpenCodeServer,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let query = [
            ("directory", project.path().to_string_lossy().into_owned()),
            ("archived", "false".to_owned()),
            ("order", "desc".to_owned()),
            ("limit", SESSION_LIMIT.to_owned()),
        ];
        let response = server.request_json(Method::GET, "/api/session", &query, None)?;
        let values = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OpenCodeError::Protocol("session list has no data array".into()))?;
        let active = server.active_sessions()?;

        let mut sessions = Vec::with_capacity(values.len());
        for value in values {
            if session_directory(value)
                .is_some_and(|directory| directory_matches(directory, project.path()))
            {
                if let Some(mut session) = session_info(value) {
                    session.is_working = active.contains(&session.id);
                    server.remember_session(&session.id);
                    sessions.push(session);
                }
            }
        }

        Ok(OpenCodeSessionList {
            project: project.info(),
            sessions,
        })
    }

    pub fn start_session(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        self.start_session_with_options(project_path, None, None)
    }

    pub fn choices(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<ProviderChoices, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let query = [(
            "location[directory]",
            project.path().to_string_lossy().into_owned(),
        )];
        let models = server.request_json(Method::GET, "/api/model", &query, None)?;
        let agents = server.request_json(Method::GET, "/api/agent", &query, None)?;
        // Provider names include custom endpoints and routed services. Keep the
        // native provider ID as a fallback if this optional metadata is unavailable.
        let providers = server
            .request_json(Method::GET, "/api/provider", &query, None)
            .ok();
        let provider_names: HashMap<&str, &str> = providers
            .as_ref()
            .and_then(|response| response.get("data"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|provider| {
                let id = provider.get("id")?.as_str()?;
                let name = provider.get("name")?.as_str()?;
                (!name.trim().is_empty()).then_some((id, name))
            })
            .collect();
        let default_model = server
            .request_json(Method::GET, "/api/model/default", &query, None)
            .ok()
            .and_then(|response| model_id(response.get("data")?));
        let models = models
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OpenCodeError::Protocol("model list has no data array".into()))?
            .iter()
            .filter(|model| model.get("enabled").and_then(Value::as_bool) == Some(true))
            .filter_map(|model| {
                let provider_id = model.get("providerID")?.as_str()?;
                Some(ProviderModelChoice {
                    id: format!("{}/{}", provider_id, model.get("id")?.as_str()?),
                    name: model.get("name")?.as_str()?.to_owned(),
                    context_window: model
                        .pointer("/limit/context")
                        .and_then(Value::as_u64)
                        .filter(|limit| *limit > 0),
                    provider: Some(ProviderChoice {
                        id: provider_id.to_owned(),
                        name: provider_names
                            .get(provider_id)
                            .copied()
                            .unwrap_or(provider_id)
                            .to_owned(),
                    }),
                    efforts: model
                        .get("variants")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|variant| {
                            let id = variant.get("id")?.as_str()?.to_owned();
                            Some(ProviderChoice {
                                name: id.clone(),
                                id,
                            })
                        })
                        .collect(),
                    default_effort: None,
                })
            })
            .collect();
        let agents = agents
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OpenCodeError::Protocol("agent list has no data array".into()))?
            .iter()
            .filter(|agent| {
                agent.get("hidden").and_then(Value::as_bool) != Some(true)
                    && agent.get("mode").and_then(Value::as_str) != Some("subagent")
            })
            .filter_map(|agent| {
                Some(ProviderChoice {
                    id: agent.get("id")?.as_str()?.to_owned(),
                    name: agent.get("name")?.as_str()?.to_owned(),
                })
            })
            .collect();
        Ok(ProviderChoices {
            models,
            agents,
            default_model,
        })
    }

    pub fn start_session_with_options(
        &self,
        project_path: impl AsRef<Path>,
        model: Option<&str>,
        agent: Option<&str>,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let model = model.map(|model| model_ref(model, None)).transpose()?;
        let response = server.request_json(
            Method::POST,
            "/api/session",
            &[],
            Some(json!({
                "location": { "directory": project.path().to_string_lossy() },
                "model": model,
                "agent": agent
            })),
        )?;
        let value = response.get("data").ok_or_else(|| {
            OpenCodeError::Protocol("session create response has no session".into())
        })?;
        if !session_directory(value)
            .is_some_and(|directory| directory_matches(directory, project.path()))
        {
            return Err(OpenCodeError::SessionNotInProject);
        }
        let session = session_info(value).ok_or_else(|| {
            OpenCodeError::Protocol("session create response has no session ID".into())
        })?;
        server.remember_session(&session.id);
        Ok(OpenCodeSessionInfo {
            project: project.info(),
            session,
        })
    }

    pub fn commands(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<Vec<OpenCodeCommandInfo>, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let query = [(
            "location[directory]",
            project.path().to_string_lossy().into_owned(),
        )];
        let response = server.request_json(Method::GET, "/api/command", &query, None)?;
        serde_json::from_value(response.get("data").cloned().unwrap_or(Value::Null))
            .map_err(|error| OpenCodeError::Protocol(format!("invalid command list: {error}")))
    }

    pub fn usage_limits(&self, project_path: impl AsRef<Path>) -> Result<Value, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let query = [(
            "location[directory]",
            project.path().to_string_lossy().into_owned(),
        )];
        let response =
            server.request_json(Method::GET, "/api/experimental/usage/limits", &query, None)?;
        response
            .get("data")
            .cloned()
            .ok_or_else(|| OpenCodeError::Protocol("usage response has no data".into()))
    }

    pub fn run_command(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        command: &OpenCodeCommandInput,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<(), OpenCodeError> {
        if command.name.trim().is_empty() {
            return Err(OpenCodeError::Protocol(
                "command name must not be empty".into(),
            ));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        select_model(&server, &project, &session, model, effort)?;
        server.request_json(
            Method::POST,
            &format!("/api/session/{session_id}/command"),
            &[],
            Some(
                serde_json::to_value(command)
                    .map_err(|error| OpenCodeError::Protocol(error.to_string()))?,
            ),
        )?;
        Ok(())
    }

    pub fn resume_session(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        Ok(OpenCodeSessionInfo {
            project: project.info(),
            session,
        })
    }

    pub fn session_history(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        cursor: Option<&str>,
    ) -> Result<SessionHistoryPage, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        let path = format!("/api/session/{session_id}/message");
        let mut query = vec![("limit", "50".to_owned())];
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor.to_owned()));
        } else {
            query.push(("order", "desc".to_owned()));
        }
        let response = server.request_json(Method::GET, &path, &query, None)?;
        let messages = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OpenCodeError::Protocol("session history has no data array".into()))?;
        Ok(SessionHistoryPage {
            is_working: Some(session.is_working),
            model: session.model,
            effort: session.effort,
            agent: session.agent,
            data: messages.iter().rev().cloned().collect(),
            next_cursor: response
                .pointer("/cursor/next")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    /// Submit a prompt. OpenCode durably admits it before this call returns;
    /// response and tool updates are delivered through `subscribe`.
    pub fn send_prompt(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        prompt: &str,
    ) -> Result<(), OpenCodeError> {
        self.send_prompt_with_id(project_path, session_id, prompt, None)
    }

    pub fn select_agent(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        agent: &str,
    ) -> Result<(), OpenCodeError> {
        if agent.trim().is_empty() {
            return Err(OpenCodeError::Protocol("agent ID must not be empty".into()));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        if session.agent.as_deref() != Some(agent) {
            server.request_json(
                Method::POST,
                &format!("/api/session/{session_id}/agent"),
                &[],
                Some(json!({ "agent": agent })),
            )?;
        }
        Ok(())
    }

    pub fn send_prompt_with_id(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        prompt: &str,
        message_id: Option<&str>,
    ) -> Result<(), OpenCodeError> {
        self.send_prompt_with_options(project_path, session_id, prompt, message_id, None, None)
    }

    pub fn send_prompt_with_options(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        prompt: &str,
        message_id: Option<&str>,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<(), OpenCodeError> {
        if prompt.trim().is_empty() {
            return Err(OpenCodeError::EmptyPrompt);
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        select_model(&server, &project, &session, model, effort)?;
        let path = format!("/api/session/{session_id}/prompt");
        server.request_json(
            Method::POST,
            &path,
            &[],
            Some(json!({ "text": prompt, "id": message_id })),
        )?;
        Ok(())
    }

    pub fn session_details(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<SessionDetails, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let changes = server
            .request_json(
                Method::GET,
                &format!("/api/session/{session_id}/diff"),
                &[("context", "0".to_owned())],
                None,
            )
            .ok()
            .and_then(|response| {
                let mut total = LineChanges::default();
                for file in response.get("data")?.as_array()? {
                    total.additions += file.get("additions")?.as_u64()?;
                    total.deletions += file.get("deletions")?.as_u64()?;
                }
                Some(total)
            });
        Ok(SessionDetails {
            // VCS metadata belongs to the directory, not this session. The
            // desktop supplies the branch recorded during session execution.
            branch: None,
            branch_is_current: false,
            changes,
        })
    }

    pub fn set_session_archived(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        archived: bool,
    ) -> Result<(), OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        if archived && session.is_working {
            return Err(OpenCodeError::Protocol(
                "stop the running turn before archiving this session".into(),
            ));
        }
        server.request_json(
            Method::PATCH,
            &format!("/api/session/{session_id}"),
            &[],
            Some(json!({ "archived": archived })),
        )?;
        if archived {
            self.inner.questions.clear(
                PROVIDER_NAME,
                &project.path().to_string_lossy(),
                Some(session_id),
            );
        }
        Ok(())
    }

    pub fn cancel_turn(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(), OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let path = format!("/api/session/{session_id}/interrupt");
        server.request_json(Method::POST, &path, &[], None)?;
        self.inner.questions.clear(
            PROVIDER_NAME,
            &project.path().to_string_lossy(),
            Some(session_id),
        );
        Ok(())
    }

    pub fn refresh_questions(&self, project_path: impl AsRef<Path>) -> Result<(), OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        sync_questions(&server, &self.inner.questions, project.path())
    }

    pub fn ensure_checkout_idle(&self, checkout: &Path) -> Result<(), OpenCodeError> {
        let servers: Vec<_> = self
            .inner
            .lock_servers()
            .iter()
            .filter(|(path, server)| {
                path.starts_with(checkout) && server.alive.load(Ordering::Acquire)
            })
            .map(|(path, server)| (path.clone(), Arc::clone(server)))
            .collect();
        for (path, server) in servers {
            if Self::list_sessions_on(&Project::open(&path)?, &server)?
                .sessions
                .iter()
                .any(|session| session.is_working)
            {
                return Err(OpenCodeError::Protocol(
                    "Stop running OpenCode sessions before removing this worktree.".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn release_checkout(&self, checkout: &Path) {
        let mut servers = self.inner.lock_servers();
        let paths: Vec<_> = servers
            .keys()
            .filter(|path| path.starts_with(checkout))
            .cloned()
            .collect();
        for path in paths {
            if let Some(server) = servers.remove(&path) {
                server.close(&self.inner.processes);
                self.inner
                    .questions
                    .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
            }
        }
    }

    /// Terminate every OpenCode server owned by this provider.
    pub fn shutdown(&self) {
        if self.inner.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        deny_pending_approvals(&self.inner.approvals, None, None);
        let servers = {
            let mut servers = self.inner.lock_servers();
            std::mem::take(&mut *servers)
        };
        for (path, server) in servers {
            server.close(&self.inner.processes);
            self.inner
                .questions
                .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
        }
        self.inner.lock_subscribers().clear();
    }

    fn session(
        &self,
        server: &OpenCodeServer,
        project: &Project,
        session_id: &str,
    ) -> Result<OpenCodeSession, OpenCodeError> {
        if !valid_session_id(session_id) {
            return Err(OpenCodeError::InvalidSessionId);
        }
        let path = format!("/api/session/{session_id}");
        let response = server.request_json(Method::GET, &path, &[], None)?;
        let value = response
            .get("data")
            .ok_or_else(|| OpenCodeError::Protocol("session response has no data".into()))?;
        if !session_directory(value)
            .is_some_and(|directory| directory_matches(directory, project.path()))
        {
            return Err(OpenCodeError::SessionNotInProject);
        }
        let mut session = session_info(value)
            .ok_or_else(|| OpenCodeError::Protocol("session response has no session ID".into()))?;
        session.is_working = server.active_sessions()?.contains(&session.id);
        server.remember_session(&session.id);
        Ok(session)
    }

    fn server(&self, project: &Project) -> Result<Arc<OpenCodeServer>, OpenCodeError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(OpenCodeError::Closed);
        }

        let mut servers = self.inner.lock_servers();
        if let Some(server) = servers.get(project.path()) {
            if server.alive.load(Ordering::Acquire) {
                return Ok(Arc::clone(server));
            }
            let dead = servers
                .remove(project.path())
                .expect("entry was just found");
            dead.close(&self.inner.processes);
        }

        if let Some(server) = self
            .inner
            .service_file
            .as_deref()
            .and_then(discover_service)
        {
            self.listen(&server, project)?;
            servers.insert(project.path().to_path_buf(), Arc::clone(&server));
            return Ok(server);
        }

        let executable = match self.inner.executable.as_deref() {
            Some(path) if is_executable_file(path) => path.to_path_buf(),
            Some(path) => return Err(OpenCodeError::ExecutableNotFound(path.to_path_buf())),
            None => find_opencode_executable()?,
        };
        let port = reserve_local_port()?;
        let startup_deadline = Instant::now() + SERVER_START_TIMEOUT;
        let port_string = port.to_string();
        let configured_password = env::var(SERVER_PASSWORD_ENV)
            .ok()
            .filter(|password| !password.is_empty());
        let mut command = Command::new(&executable);
        command
            .args(["serve", "--hostname", "127.0.0.1", "--port", &port_string])
            .current_dir(project.path())
            .stdin(Stdio::null())
            .stdout(if configured_password.is_some() {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|source| OpenCodeError::StartServer {
                executable: executable.clone(),
                source,
            })?;
        let password_receiver = if configured_password.is_none() {
            child.stdout.take().map(capture_server_password)
        } else {
            None
        };
        let process_id = self.inner.processes.register(child)?;
        let password = match configured_password {
            Some(password) => password,
            None => {
                let Some(receiver) = password_receiver else {
                    let _ = self.inner.processes.terminate(process_id);
                    return Err(OpenCodeError::ServerPasswordUnavailable(
                        "OpenCode stdout was not available".to_owned(),
                    ));
                };
                let timeout = startup_deadline.saturating_duration_since(Instant::now());
                match receiver.recv_timeout(timeout) {
                    Ok(Ok(password)) => password,
                    Ok(Err(message)) => {
                        let _ = self.inner.processes.terminate(process_id);
                        return Err(OpenCodeError::ServerPasswordUnavailable(message));
                    }
                    Err(error) => {
                        let _ = self.inner.processes.terminate(process_id);
                        return Err(OpenCodeError::ServerPasswordUnavailable(error.to_string()));
                    }
                }
            }
        };
        let server = Arc::new(
            OpenCodeServer::new(
                Some(process_id),
                format!("http://127.0.0.1:{port}"),
                Some(password),
            )
            .inspect_err(|_| {
                let _ = self.inner.processes.terminate(process_id);
            })?,
        );

        let query = [("directory", project.path().to_string_lossy().into_owned())];
        loop {
            let readiness = match server.request_json(Method::GET, "/api/session", &query, None) {
                Ok(response) if response.get("data").and_then(Value::as_array).is_some() => Ok(()),
                Ok(_) => Err(OpenCodeError::Protocol(
                    "session list response has no data array".to_owned(),
                )),
                Err(error) => Err(error),
            };
            match readiness {
                Ok(()) => break,
                Err(OpenCodeError::AuthenticationRequired) => {
                    let _ = self.inner.processes.terminate(process_id);
                    return Err(OpenCodeError::AuthenticationRequired);
                }
                Err(error) if Instant::now() >= startup_deadline => {
                    let _ = self.inner.processes.terminate(process_id);
                    return Err(OpenCodeError::ServerNotReady(error.to_string()));
                }
                Err(_) => thread::sleep(SERVER_POLL_INTERVAL),
            }
        }

        self.listen(&server, project)?;
        servers.insert(project.path().to_path_buf(), Arc::clone(&server));
        Ok(server)
    }

    fn listen(&self, server: &Arc<OpenCodeServer>, project: &Project) -> Result<(), OpenCodeError> {
        let result = if self.inner.closed.load(Ordering::Acquire) {
            Err(OpenCodeError::Closed)
        } else {
            spawn_event_listener(
                Arc::clone(server),
                project.path().to_path_buf(),
                self.inner.approvals.clone(),
                self.inner.questions.clone(),
                Arc::clone(&self.inner.subscribers),
            )
        };
        if result.is_err() {
            server.close(&self.inner.processes);
        } else {
            // The stream is live-only. Recover forms that predate this connection.
            // A missing form API must not prevent older servers from connecting.
            let _ = sync_questions(server, &self.inner.questions, project.path());
        }
        result
    }
}

fn select_model(
    server: &OpenCodeServer,
    project: &Project,
    session: &OpenCodeSession,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<(), OpenCodeError> {
    if model.is_none() && effort.is_none() {
        return Ok(());
    }
    let selected_model = model.or(session.model.as_deref());
    let fallback = if selected_model.is_none() {
        let query = [(
            "location[directory]",
            project.path().to_string_lossy().into_owned(),
        )];
        let response = server.request_json(Method::GET, "/api/model/default", &query, None)?;
        response.get("data").and_then(model_id)
    } else {
        None
    };
    let selected_model = selected_model.or(fallback.as_deref()).ok_or_else(|| {
        OpenCodeError::Protocol("no model available for the selected effort".into())
    })?;
    // Changing models resets the variant; an empty effort explicitly selects the base model.
    let selected_effort = effort.unwrap_or_else(|| {
        if model.is_some_and(|model| session.model.as_deref() != Some(model)) {
            ""
        } else {
            session.effort.as_deref().unwrap_or("")
        }
    });
    if session.model.as_deref() != Some(selected_model)
        || session.effort.as_deref().unwrap_or("") != selected_effort
    {
        server.request_json(
            Method::POST,
            &format!("/api/session/{}/model", session.id),
            &[],
            Some(json!({ "model": model_ref(selected_model, Some(selected_effort))? })),
        )?;
    }
    Ok(())
}

impl OpenCodeServer {
    fn new(
        process_id: Option<ProcessId>,
        base_url: String,
        password: Option<String>,
    ) -> Result<Self, OpenCodeError> {
        let builder = || {
            Client::builder()
                .connect_timeout(Duration::from_secs(1))
                .redirect(reqwest::redirect::Policy::none())
        };
        Ok(Self {
            process_id,
            base_url,
            client: builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(|error| OpenCodeError::Request(error.to_string()))?,
            event_client: builder()
                .build()
                .map_err(|error| OpenCodeError::Request(error.to_string()))?,
            password,
            alive: Arc::new(AtomicBool::new(true)),
            session_ids: Mutex::new(HashSet::new()),
            pending_permission_ids: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    fn authenticate(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match &self.password {
            Some(password) => request.basic_auth("opencode", Some(password)),
            None => request,
        }
    }

    fn close(&self, processes: &ProcessRegistry) {
        self.alive.store(false, Ordering::Release);
        if let Some(id) = self.process_id {
            let _ = processes.terminate(id);
        }
    }

    fn request_json(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, OpenCodeError> {
        let url = api_url(&self.base_url, path, query)?;
        let mut request = self.authenticate(self.client.request(method, url));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .map_err(|error| OpenCodeError::Request(error.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .map_err(|error| OpenCodeError::Request(error.to_string()))?;
        decode_json_response(status, &bytes)
    }

    fn remember_session(&self, session_id: &str) {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(session_id.to_owned());
    }

    fn active_sessions(&self) -> Result<HashSet<String>, OpenCodeError> {
        // Activity belongs to the runtime drain, not persisted busy/idle times.
        // A running session can have no busy timestamp (or a previous idle time).
        let response = self.request_json(Method::GET, "/api/session/active", &[], None)?;
        let active = response
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| OpenCodeError::Protocol("active sessions has no data map".into()))?;
        Ok(active.keys().cloned().collect())
    }

    fn owns_session(&self, session_id: &str) -> bool {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(session_id)
    }
}

fn question_request(
    server: &Arc<OpenCodeServer>,
    project: &Path,
    form: &Value,
) -> Result<(QuestionPrompt, Reply), OpenCodeError> {
    let session = form["sessionID"]
        .as_str()
        .filter(|id| valid_session_id(id))
        .ok_or_else(|| OpenCodeError::Protocol("form has no valid session ID".into()))?;
    let id = form["id"]
        .as_str()
        .filter(|id| {
            id.starts_with("frm_")
                && id.len() > 4
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .ok_or_else(|| OpenCodeError::Protocol("form has no valid ID".into()))?;
    let fields = form["fields"]
        .as_array()
        .ok_or_else(|| OpenCodeError::Protocol("form has no fields".into()))?;
    let prompt = QuestionPrompt {
        provider: PROVIDER_NAME.into(),
        project_path: project.to_string_lossy().into_owned(),
        session_id: session.into(),
        request_id: id.into(),
        title: form["title"].as_str().unwrap_or("Questions").into(),
        fields: fields.clone(),
    };
    let path = format!("/api/session/{session}/form/{id}");
    server.remember_session(session);
    let server = Arc::clone(server);
    let reply: Reply = Arc::new(move |answer| {
        if !server.alive.load(Ordering::Acquire) {
            return Err("OpenCode disconnected. Reconnect before answering.".into());
        }
        let result = match answer {
            Some(answer) => server.request_json(
                Method::POST,
                &format!("{path}/reply"),
                &[],
                Some(json!({ "answer": answer })),
            ),
            None => server.request_json(Method::DELETE, &path, &[], None),
        };
        result.map(|_| ()).map_err(|error| error.to_string())
    });
    Ok((prompt, reply))
}

fn sync_questions(
    server: &Arc<OpenCodeServer>,
    questions: &QuestionBroker,
    project: &Path,
) -> Result<(), OpenCodeError> {
    let project_path = project.to_string_lossy();
    for _ in 0..2 {
        let revision = questions.revision();
        let response = server.request_json(
            Method::GET,
            "/api/form",
            &[("location[directory]", project_path.to_string())],
            None,
        )?;
        let forms = response["data"]
            .as_array()
            .ok_or_else(|| OpenCodeError::Protocol("form list has no data array".into()))?;
        let requests = forms
            .iter()
            // Global MCP forms do not belong to a conversation.
            .filter(|form| form["sessionID"].as_str().is_some_and(valid_session_id))
            .map(|form| question_request(server, project, form))
            .collect::<Result<Vec<_>, _>>()?;
        if !server.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        if questions
            .sync(PROVIDER_NAME, &project_path, revision, requests)
            .map_err(OpenCodeError::Protocol)?
        {
            break;
        }
    }
    Ok(())
}

fn spawn_event_listener(
    server: Arc<OpenCodeServer>,
    project_path: PathBuf,
    approvals: ApprovalBroker,
    questions: QuestionBroker,
    subscribers: EventSubscribers,
) -> Result<(), OpenCodeError> {
    let (ready, receiver) = mpsc::channel();
    thread::spawn(move || {
        let url = match api_url(&server.base_url, "/api/event", &[]) {
            Ok(url) => url,
            Err(_) => return,
        };
        let request = server.authenticate(server.event_client.get(url));
        let result = request.send();
        let Ok(response) = result else {
            publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
            return;
        };
        if !response.status().is_success() {
            publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
            return;
        }

        let mut reader = BufReader::new(response);
        let mut ready = Some(ready);
        let _ = read_sse_frames(
            &mut reader,
            || server.alive.load(Ordering::Acquire),
            |payload| {
                let Ok(event) = parse_event(&payload, &project_path) else {
                    return;
                };
                // Wait for the stream's first frame before admitting prompts or
                // fetching snapshots: SSE subscriptions have no replay.
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
                if !server.alive.load(Ordering::Acquire) {
                    return;
                }
                // A resolution can arrive during the first snapshot, before
                // its session is known locally. Invalidate that snapshot even
                // when a location-less event cannot otherwise be routed yet.
                if matches!(event.event_type.as_str(), "form.replied" | "form.cancelled")
                    && event
                        .location
                        .as_ref()
                        .and_then(|location| location["directory"].as_str())
                        .map_or(true, |directory| {
                            directory_matches(directory, &project_path)
                        })
                {
                    if let (Some(session), Some(id)) =
                        (event.data["sessionID"].as_str(), event.data["id"].as_str())
                    {
                        questions.dismiss(
                            PROVIDER_NAME,
                            &project_path.to_string_lossy(),
                            session,
                            id,
                        );
                    }
                }
                if !event_belongs_to_project(
                    event.location.as_ref(),
                    event.data.get("form").unwrap_or(&event.data),
                    &project_path,
                    &server,
                ) {
                    return;
                }

                if event.event_type == "permission.asked" {
                    request_permission(
                        &server,
                        &approvals,
                        &project_path,
                        &subscribers,
                        &event.data,
                    );
                }
                match event.event_type.as_str() {
                    "form.created" => {
                        if let Ok((prompt, reply)) =
                            question_request(&server, &project_path, &event.data["form"])
                        {
                            let _ = questions.insert(prompt, reply);
                        }
                    }
                    "session.deleted" | "session.archived" | "session.execution.interrupted" => {
                        if let Some(session) = event.data["sessionID"].as_str() {
                            questions.clear(
                                PROVIDER_NAME,
                                &project_path.to_string_lossy(),
                                Some(session),
                            );
                        }
                    }
                    _ => {}
                }
                publish_event(&subscribers, event);
            },
        );
        publish_disconnected(&server, &project_path, &approvals, &questions, &subscribers);
    });
    receiver
        .recv_timeout(SERVER_START_TIMEOUT)
        .map_err(|error| OpenCodeError::ServerNotReady(format!("event subscription: {error}")))
}

fn service_file() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .map(|state| state.join("opencode/service.json"))
}

fn discover_service(file: &Path) -> Option<Arc<OpenCodeServer>> {
    let registration: Value = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    let url = Url::parse(registration.get("url")?.as_str()?).ok()?;
    let host = url.host_str()?;
    let local = host == "localhost"
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if !local || !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let server = OpenCodeServer::new(
        None,
        url.to_string(),
        registration
            .get("password")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .ok()?;
    let response = server
        .authenticate(
            server
                .client
                .get(api_url(&server.base_url, "/api/info", &[]).ok()?),
        )
        .timeout(Duration::from_secs(2))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let info: Value = response.json().ok()?;
    let version = info.get("version")?.as_str()?;
    if info.get("pid")?.as_u64()? != registration.get("pid")?.as_u64()?
        || !version.starts_with("2.")
        || registration
            .get("version")
            .and_then(Value::as_str)
            .is_some_and(|registered| registered != version)
    {
        return None;
    }
    Some(Arc::new(server))
}

fn request_permission(
    server: &OpenCodeServer,
    approvals: &ApprovalBroker,
    project_path: &Path,
    subscribers: &EventSubscribers,
    data: &Value,
) {
    let Some(session_id) = data.get("sessionID").and_then(Value::as_str) else {
        return;
    };
    let Some(permission_id) = data.get("id").and_then(Value::as_str) else {
        return;
    };
    if !valid_session_id(session_id) || !valid_permission_id(permission_id) {
        return;
    }
    let action = data
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("tool action");
    let resources = data
        .get("resources")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let message = data
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let description = [message, resources.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let request_id = format!("{session_id}:{permission_id}");
    let request = PermissionRequest {
        provider: PROVIDER_NAME.to_owned(),
        project_path: project_path.to_string_lossy().into_owned(),
        session_id: Some(session_id.to_owned()),
        request_id,
        summary: format!("OpenCode requests permission for {action}"),
        description: if description.is_empty() {
            "OpenCode requested permission to continue.".to_owned()
        } else {
            description
        },
        provider_details: json!({
            "sessionId": session_id,
            "permissionId": permission_id,
            "projectPath": project_path.to_string_lossy(),
            "request": data,
        }),
    };
    let Ok(waiter) = approvals.request(request) else {
        return;
    };

    server
        .pending_permission_ids
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(format!("{session_id}:{permission_id}"));
    let server = server.clone_for_permission();
    let subscribers = Arc::clone(subscribers);
    let project_path = project_path.to_path_buf();
    let session_id = session_id.to_owned();
    let permission_id = permission_id.to_owned();
    let request_id = format!("{session_id}:{permission_id}");
    thread::spawn(move || {
        let decision = waiter.wait(APPROVAL_TIMEOUT);
        let decision = permission_reply(decision);
        let path = format!("/api/session/{session_id}/permission/{permission_id}/reply");
        let result = server.request_json(
            Method::POST,
            &path,
            &[],
            Some(json!({ "decision": decision })),
        );
        server
            .pending_permission_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&request_id);
        if let Err(error) = result {
            publish_event(
                &subscribers,
                OpenCodeEvent {
                    project_path: project_path.to_string_lossy().into_owned(),
                    event_type: "permission.reply.failed".to_owned(),
                    id: None,
                    created: None,
                    data: json!({ "sessionID": session_id, "message": error.to_string() }),
                    location: Some(json!({ "directory": project_path.to_string_lossy() })),
                },
            );
        }
    });
}

impl OpenCodeServer {
    fn clone_for_permission(&self) -> Self {
        Self {
            process_id: self.process_id,
            base_url: self.base_url.clone(),
            client: self.client.clone(),
            event_client: self.event_client.clone(),
            password: self.password.clone(),
            alive: Arc::clone(&self.alive),
            session_ids: Mutex::new(
                self.session_ids
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone(),
            ),
            pending_permission_ids: Arc::clone(&self.pending_permission_ids),
        }
    }
}

fn event_belongs_to_project(
    location: Option<&Value>,
    data: &Value,
    project_path: &Path,
    server: &OpenCodeServer,
) -> bool {
    if let Some(directory) = location
        .and_then(|location| location.get("directory"))
        .and_then(Value::as_str)
    {
        return directory_matches(directory, project_path);
    }
    data.get("sessionID")
        .and_then(Value::as_str)
        .is_some_and(|session_id| server.owns_session(session_id))
}

fn publish_disconnected(
    server: &OpenCodeServer,
    project_path: &Path,
    approvals: &ApprovalBroker,
    questions: &QuestionBroker,
    subscribers: &EventSubscribers,
) {
    if server.alive.load(Ordering::Acquire) {
        questions.clear(PROVIDER_NAME, &project_path.to_string_lossy(), None);
    }
    if server.alive.swap(false, Ordering::AcqRel) {
        let pending_ids = server
            .pending_permission_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let project_path = project_path.to_string_lossy().into_owned();
        deny_pending_approvals(approvals, Some(&project_path), Some(&pending_ids));
        publish_event(
            subscribers,
            OpenCodeEvent {
                project_path: project_path.clone(),
                event_type: "server.disconnected".to_owned(),
                id: None,
                created: None,
                data: Value::Null,
                location: Some(json!({ "directory": project_path })),
            },
        );
    }
}

fn permission_reply(decision: ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::AllowOnce => "once",
        ApprovalDecision::Deny => "reject",
    }
}

fn deny_pending_approvals(
    approvals: &ApprovalBroker,
    project_path: Option<&str>,
    request_ids: Option<&HashSet<String>>,
) {
    for request in approvals.pending().into_iter().filter(|request| {
        if request.provider != PROVIDER_NAME {
            return false;
        }
        project_path.map_or(true, |path| request.project_path == path)
            && request_ids.map_or(true, |request_ids| {
                request_ids.contains(&request.request_id)
            })
    }) {
        let _ = approvals.resolve(
            PROVIDER_NAME,
            &request.project_path,
            &request.request_id,
            ApprovalDecision::Deny,
        );
    }
}

fn read_sse_frames<R, F>(
    reader: &mut R,
    mut active: impl FnMut() -> bool,
    mut on_data: F,
) -> io::Result<()>
where
    R: BufRead,
    F: FnMut(String),
{
    let mut data = Vec::new();
    while active() {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            if !data.is_empty() {
                on_data(data.join("\n"));
            }
            return Ok(());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !data.is_empty() {
                on_data(data.join("\n"));
                data.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        if field == "data" {
            data.push(value.strip_prefix(' ').unwrap_or(value).to_owned());
        }
    }
    Ok(())
}

fn capture_server_password(stdout: ChildStdout) -> Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut sent_password = false;
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    if !sent_password {
                        if let Some(password) = line.strip_prefix("server password ") {
                            sent_password = true;
                            if sender.send(Ok(password.to_owned())).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    if !sent_password {
                        let _ = sender.send(Err(error.to_string()));
                    }
                    return;
                }
            }
        }
        if !sent_password {
            let _ = sender.send(Err(
                "OpenCode exited without printing its generated server password".to_owned(),
            ));
        }
    });
    receiver
}

fn api_url(base_url: &str, path: &str, query: &[(&str, String)]) -> Result<Url, OpenCodeError> {
    let mut url = Url::parse(base_url)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid server URL: {error}")))?;
    url.set_path(path);
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (name, value) in query {
            pairs.append_pair(name, value);
        }
    }
    Ok(url)
}

fn decode_json_response(status: StatusCode, body: &[u8]) -> Result<Value, OpenCodeError> {
    if status == StatusCode::UNAUTHORIZED {
        return Err(OpenCodeError::AuthenticationRequired);
    }
    if !status.is_success() {
        let message = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .or_else(|| value.get("error"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| String::from_utf8_lossy(body).trim().to_owned())
            .chars()
            .take(600)
            .collect();
        return Err(OpenCodeError::HttpStatus {
            status: status.as_u16(),
            message,
        });
    }
    if body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(body)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid JSON response: {error}")))
}

fn parse_event(payload: &str, project_path: &Path) -> Result<OpenCodeEvent, OpenCodeError> {
    let value = serde_json::from_str::<Value>(payload)
        .map_err(|error| OpenCodeError::Protocol(format!("invalid SSE event JSON: {error}")))?;
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| OpenCodeError::Protocol("SSE event has no type".to_owned()))?;
    Ok(OpenCodeEvent {
        project_path: project_path.to_string_lossy().into_owned(),
        event_type: event_type.to_owned(),
        id: value.get("id").and_then(Value::as_str).map(str::to_owned),
        created: value.get("created").and_then(Value::as_u64),
        data: value.get("data").cloned().unwrap_or(Value::Null),
        location: value.get("location").cloned(),
    })
}

fn model_ref(model: &str, effort: Option<&str>) -> Result<Value, OpenCodeError> {
    model
        .split_once('/')
        .filter(|(provider, id)| !provider.is_empty() && !id.is_empty())
        .map(|(provider, id)| {
            let mut model = json!({ "providerID": provider, "id": id });
            if let Some(effort) = effort.filter(|effort| !effort.is_empty()) {
                model["variant"] = json!(effort);
            }
            model
        })
        .ok_or_else(|| OpenCodeError::Protocol("model must include its provider and ID".into()))
}

fn model_id(model: &Value) -> Option<String> {
    Some(format!(
        "{}/{}",
        model.get("providerID")?.as_str()?,
        model.get("id")?.as_str()?
    ))
}

fn session_info(value: &Value) -> Option<OpenCodeSession> {
    Some(OpenCodeSession {
        agent: value
            .get("agent")
            .and_then(Value::as_str)
            .map(str::to_owned),
        model: value.get("model").and_then(model_id),
        effort: Some(
            value
                .pointer("/model/variant")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        ),
        is_working: false,
        id: value.get("id")?.as_str()?.to_owned(),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        updated_at: value
            .get("time")
            .and_then(|time| time.get("updated"))
            .and_then(Value::as_u64),
    })
}

fn session_directory(value: &Value) -> Option<&str> {
    value.get("location")?.get("directory")?.as_str()
}

fn directory_matches(directory: &str, project_path: &Path) -> bool {
    Path::new(directory)
        .canonicalize()
        .is_ok_and(|canonical| canonical == project_path)
}

fn valid_session_id(value: &str) -> bool {
    value.starts_with("ses")
        && value.len() > 3
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn valid_permission_id(value: &str) -> bool {
    value.starts_with("per")
        && value.len() > 3
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn publish_event(subscribers: &EventSubscribers, event: OpenCodeEvent) {
    subscribers
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|subscriber| subscriber.send(event.clone()).is_ok());
}

fn reserve_local_port() -> Result<u16, OpenCodeError> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(OpenCodeError::PortAssignment)?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(OpenCodeError::PortAssignment)
}

fn find_opencode_executable() -> Result<PathBuf, OpenCodeError> {
    let executable_name = if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    };
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
            ".opencode/bin/opencode",
            ".local/bin/opencode",
            ".npm-global/bin/opencode",
            ".bun/bin/opencode",
        ] {
            let candidate = home.join(relative);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }
    for candidate in ["/opt/homebrew/bin/opencode", "/usr/local/bin/opencode"] {
        let candidate = PathBuf::from(candidate);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    Err(OpenCodeError::NotInstalled)
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

impl ProviderInner {
    fn lock_servers(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<OpenCodeServer>>> {
        self.servers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_subscribers(&self) -> MutexGuard<'_, Vec<Sender<OpenCodeEvent>>> {
        self.subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for ProviderInner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        deny_pending_approvals(&self.approvals, None, None);
        let servers = self
            .servers
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (path, server) in std::mem::take(servers) {
            server.close(&self.processes);
            self.questions
                .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
        }
    }
}

#[derive(Debug, Error)]
pub enum OpenCodeError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Process(#[from] crate::processes::ProcessError),
    #[error("OpenCode CLI was not found. Install OpenCode and ensure `opencode` is on PATH, then restart Caelum.")]
    NotInstalled,
    #[error("OpenCode executable was not found at '{0}'. Check the configured OpenCode CLI path.")]
    ExecutableNotFound(PathBuf),
    #[error("could not start OpenCode server using '{executable}': {source}")]
    StartServer {
        executable: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not reserve a local port for the OpenCode server: {0}")]
    PortAssignment(#[source] io::Error),
    #[error("OpenCode server did not become ready: {0}")]
    ServerNotReady(String),
    #[error("could not obtain the OpenCode server password: {0}")]
    ServerPasswordUnavailable(String),
    #[error("OpenCode rejected the credentials Caelum obtained. Check OPENCODE_SERVER_PASSWORD and restart Caelum.")]
    AuthenticationRequired,
    #[error("OpenCode HTTP request failed: {0}")]
    Request(String),
    #[error("OpenCode server returned HTTP {status}: {message}")]
    HttpStatus { status: u16, message: String },
    #[error("OpenCode returned an invalid response: {0}")]
    Protocol(String),
    #[error("OpenCode session ID is invalid")]
    InvalidSessionId,
    #[error("OpenCode session does not belong to this project")]
    SessionNotInProject,
    #[error("prompt cannot be empty")]
    EmptyPrompt,
    #[error("OpenCode provider is shutting down")]
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "caelum-opencode-test-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn sse_reader_handles_comments_multiline_data_and_final_unterminated_frame() {
        let input = b": keepalive\nevent: ignored\ndata: {\ndata: \"type\":\"session.text.delta\"}\n\ndata: final";
        let mut reader = BufReader::new(&input[..]);
        let mut frames = Vec::new();

        read_sse_frames(&mut reader, || true, |frame| frames.push(frame)).unwrap();

        assert_eq!(frames, ["{\n\"type\":\"session.text.delta\"}", "final"]);
    }

    #[test]
    fn session_ids_cannot_escape_the_session_route() {
        assert!(valid_session_id("ses_e7b5213dbffeaVPrGggxUYkwrK"));
        assert!(!valid_session_id("ses/other"));
        assert!(!valid_session_id("ses"));
    }

    #[test]
    fn event_parser_maps_open_code_sse_payload_to_the_desktop_contract() {
        let event = parse_event(
            r#"{"id":"evt_1","created":123,"type":"session.text.delta","location":{"directory":"/workspace"},"data":{"sessionID":"ses_1","delta":"hi"}}"#,
            Path::new("/workspace"),
        )
        .unwrap();

        assert_eq!(event.event_type, "session.text.delta");
        assert_eq!(event.id.as_deref(), Some("evt_1"));
        assert_eq!(event.created, Some(123));
        assert_eq!(event.project_path, "/workspace");
        assert_eq!(event.data["sessionID"], "ses_1");
        assert_eq!(event.location.as_ref().unwrap()["directory"], "/workspace");
    }

    #[test]
    fn unauthorized_responses_have_actionable_setup_guidance() {
        let error = decode_json_response(StatusCode::UNAUTHORIZED, b"{}")
            .unwrap_err()
            .to_string();
        assert!(error.contains(SERVER_PASSWORD_ENV));
    }

    #[test]
    fn permission_replies_never_grant_persistent_access() {
        assert_eq!(permission_reply(ApprovalDecision::AllowOnce), "once");
        assert_eq!(permission_reply(ApprovalDecision::Deny), "reject");
    }

    #[test]
    fn server_disconnect_denies_only_its_opencode_approval_requests() {
        let approvals = ApprovalBroker::default();
        let disconnected_waiter = approvals
            .request(PermissionRequest {
                provider: PROVIDER_NAME.to_owned(),
                project_path: "/workspace/one".to_owned(),
                session_id: Some("ses_1".to_owned()),
                request_id: "ses_1:per_1".to_owned(),
                summary: "Run a tool".to_owned(),
                description: "OpenCode requested permission.".to_owned(),
                provider_details: json!({ "projectPath": "/workspace/one" }),
            })
            .unwrap();
        let other_project_waiter = approvals
            .request(PermissionRequest {
                provider: PROVIDER_NAME.to_owned(),
                project_path: "/workspace/two".to_owned(),
                session_id: Some("ses_2".to_owned()),
                request_id: "ses_2:per_2".to_owned(),
                summary: "Run another tool".to_owned(),
                description: "Another OpenCode project requested permission.".to_owned(),
                provider_details: json!({ "projectPath": "/workspace/two" }),
            })
            .unwrap();
        let other_waiter = approvals
            .request(PermissionRequest {
                provider: "other".to_owned(),
                project_path: "/workspace/other".to_owned(),
                session_id: None,
                request_id: "request-1".to_owned(),
                summary: "Run another tool".to_owned(),
                description: "Another provider requested permission.".to_owned(),
                provider_details: Value::Null,
            })
            .unwrap();

        let pending_ids = HashSet::from(["ses_1:per_1".to_owned()]);
        deny_pending_approvals(&approvals, Some("/workspace/one"), Some(&pending_ids));

        assert_eq!(
            disconnected_waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::Deny
        );
        assert_eq!(approvals.pending().len(), 2);
        approvals
            .resolve(
                PROVIDER_NAME,
                "/workspace/two",
                "ses_2:per_2",
                ApprovalDecision::AllowOnce,
            )
            .unwrap();
        assert_eq!(
            other_project_waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::AllowOnce
        );
        approvals
            .resolve(
                "other",
                "/workspace/other",
                "request-1",
                ApprovalDecision::AllowOnce,
            )
            .unwrap();
        assert_eq!(
            other_waiter.wait(Duration::from_millis(10)),
            ApprovalDecision::AllowOnce
        );
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_terminates_and_unregisters_the_owned_server_process() {
        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();
        let provider =
            OpenCodeProvider::new(Arc::clone(&processes), approvals, QuestionBroker::default());
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let process_id = processes.register(child).unwrap();
        let client = Client::builder().build().unwrap();
        let project_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let server = Arc::new(OpenCodeServer {
            process_id: Some(process_id),
            base_url: "http://127.0.0.1:1".to_owned(),
            client: client.clone(),
            event_client: client,
            password: Some("unused-test-password".to_owned()),
            alive: Arc::new(AtomicBool::new(true)),
            session_ids: Mutex::new(HashSet::new()),
            pending_permission_ids: Arc::new(Mutex::new(HashSet::new())),
        });
        provider
            .inner
            .lock_servers()
            .insert(project_path, Arc::clone(&server));

        provider.shutdown();

        assert!(!server.alive.load(Ordering::Acquire));
        assert!(matches!(
            processes.terminate(process_id),
            Err(crate::processes::ProcessError::UnknownProcess)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn managed_server_lists_creates_restores_prompts_streams_and_shuts_down() {
        if Command::new("python3").arg("--version").output().is_err() {
            return;
        }

        let directory = TestDirectory::new();
        let project_path = directory.0.join("project");
        fs::create_dir_all(&project_path).unwrap();
        let executable = directory.0.join("fake-opencode");
        crate::test_support::write_executable(
            &executable,
            r##"#!/usr/bin/env python3
import json
import base64
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit, parse_qs

PROJECT = os.getcwd()
PASSWORD = os.environ.get("OPENCODE_SERVER_PASSWORD") or "fake-secret"
SESSION = {"id": "ses_fake", "title": "Fake session", "time": {"updated": 123}, "location": {"directory": PROJECT}, "model": {"providerID": "local", "id": "original"}, "agent": "build"}
FORMS = {name: {"id": name, "sessionID": "ses_fake", "title": "Questions", "fields": [
    {"key": "q0", "type": "multiselect", "title": "Targets", "custom": True, "options": [{"label": "macOS", "value": "macOS"}], "required": True}
]} for name in ["frm_live", "frm_saved", "frm_external", "frm_cancelled"]}

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def authorized(self):
        expected = "Basic " + base64.b64encode(("opencode:" + PASSWORD).encode()).decode()
        if self.headers.get("Authorization") == expected:
            return True
        self.send_error(401)
        return False

    def send_json(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if not self.authorized():
            return
        path = urlsplit(self.path).path
        if path == "/api/event":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            events = [
                {"type": "form.created", "location": {"directory": PROJECT}, "data": {"form": FORMS["frm_live"]}},
                {"type": "form.created", "location": {"directory": os.path.dirname(PROJECT)}, "data": {"form": dict(FORMS["frm_live"], id="frm_foreign")}},
                {"id": "evt_fake", "type": "session.text.delta", "location": {"directory": PROJECT}, "data": {"sessionID": "ses_fake", "assistantMessageID": "msg_fake", "delta": "hello"}},
                {"id": "evt_permission", "type": "permission.asked", "location": {"directory": PROJECT}, "data": {"id": "per_fake", "sessionID": "ses_fake", "action": "bash", "resources": ["echo hello"], "message": "Run a command?"}}
            ]
            for event in events:
                self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
            self.wfile.flush()
            try:
                while True:
                    time.sleep(0.05)
                    marker = os.path.join(PROJECT, "external-form-resolution")
                    if os.path.exists(marker):
                        os.unlink(marker)
                        for name, kind in [("frm_external", "form.replied"), ("frm_cancelled", "form.cancelled")]:
                            FORMS.pop(name, None)
                            # No location: snapshots/created events establish ownership.
                            event = {"type": kind, "data": {"id": name, "sessionID": "ses_fake", "answer": {"q0": ["external"]}}}
                            self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                    self.wfile.write(b": keepalive\n\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
        elif path == "/api/form":
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            self.send_json({"data": list(FORMS.values()) + [{"id": "frm_global", "sessionID": "global", "fields": []}]})
        elif path == "/api/session":
            assert parse_qs(urlsplit(self.path).query).get("archived", ["false"]) == ["false"]
            self.send_json({"data": [] if SESSION.get("archived") else [SESSION], "cursor": {"previous": None, "next": None}})
        elif path == "/api/session/active":
            self.send_json({"data": {"ses_fake": {"type": "running"}} if os.path.exists(os.path.join(PROJECT, "busy")) else {}})
        elif path == "/api/session/ses_fake":
            session = dict(SESSION)
            self.send_json({"data": session})
        elif path == "/api/session/ses_foreign":
            self.send_json({"data": {"id": "ses_foreign", "location": {"directory": os.path.dirname(PROJECT)}}})
        elif path == "/api/vcs":
            assert parse_qs(urlsplit(self.path).query)["location[directory]"] == [PROJECT]
            if os.path.exists(os.path.join(PROJECT, "no-vcs")):
                self.send_error(404)
                return
            self.send_json({"data": {"branch": {"current": "feature/current"}}})
        elif path == "/api/session/ses_fake/diff":
            assert parse_qs(urlsplit(self.path).query) == {"context": ["0"]}
            if os.path.exists(os.path.join(PROJECT, "no-vcs")):
                self.send_error(404)
                return
            self.send_json({"data": [{"additions": 12, "deletions": 2}, {"additions": 3, "deletions": 5}]})
        elif path == "/api/session/ses_fake/message":
            query = parse_qs(urlsplit(self.path).query)
            if "cursor" in query:
                assert query["cursor"] == ["older+opaque/=="] and "order" not in query
                self.send_json({"data": [{"id": "msg_old", "type": "user", "text": "Oldest"}], "cursor": {"previous": None, "next": None}})
                return
            assert query["order"] == ["desc"] and query["limit"] == ["50"]
            self.send_json({"data": [
                {"id": "msg_assistant", "type": "assistant", "content": [{"type": "text", "text": "Earlier answer"}, {"id": "tool_1", "type": "tool", "name": "bash", "state": {"status": "completed", "input": {"command": "cargo test"}, "content": [{"type": "text", "text": "All passed"}]}}]},
                {"id": "msg_user", "type": "user", "text": "Earlier prompt"}
            ], "cursor": {"previous": None, "next": "older+opaque/=="}})
        elif path == "/api/model":
            self.send_json({"data": [
                {"id": "model/a", "providerID": "local", "name": "Model A", "enabled": True, "limit": {"context": 200000}, "variants": [{"id": "low"}, {"id": "high"}]},
                {"id": "model/a", "providerID": "router", "canonical": "local", "name": "Model A", "enabled": True, "limit": {"context": 0}},
                {"id": "model/a", "providerID": "custom", "name": "Model A", "enabled": True},
                {"id": "disabled", "providerID": "local", "name": "Hidden", "enabled": False}
            ]})
        elif path == "/api/provider":
            assert parse_qs(urlsplit(self.path).query)["location[directory]"] == [PROJECT]
            if os.path.exists(os.path.join(PROJECT, "no-providers")):
                self.send_error(503)
                return
            self.send_json({"data": [{"id": "local", "name": "Local gateway"}, {"id": "router", "name": "OpenRouter"}]})
        elif path == "/api/model/default":
            self.send_json({"data": {"providerID": "local", "id": "model/a"}})
        elif path == "/api/agent":
            self.send_json({"data": [{"id": "build", "name": "Build", "mode": "primary", "hidden": False}, {"id": "plan", "name": "Plan", "mode": "primary", "hidden": False}, {"id": "child", "name": "Child", "mode": "subagent", "hidden": False}, {"id": "hidden", "name": "Hidden", "mode": "primary", "hidden": True}]})
        elif path in ["/api/command", "/api/experimental/usage/limits"]:
            assert parse_qs(urlsplit(self.path).query) == {"location[directory]": [PROJECT]}
            if os.path.exists(os.path.join(PROJECT, "command-api-error")):
                self.send_error(503)
                return
            if path == "/api/command":
                data = [{"name": "goal", "description": "Set or control an objective"}, {"name": "team/review"}]
            else:
                usage = os.path.join(PROJECT, "usage.json")
                with open(usage) as source:
                    data = json.load(source)
            self.send_json({"location": {"directory": PROJECT}, "data": data})
        else:
            self.send_error(404)

    def do_PATCH(self):
        if not self.authorized(): return
        assert urlsplit(self.path).path == "/api/session/ses_fake"
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        assert set(body) == {"archived"} and isinstance(body["archived"], bool)
        if os.path.exists(os.path.join(PROJECT, "archive-error")):
            self.send_error(500)
            return
        SESSION["archived"] = body["archived"]
        self.send_response(204)
        self.end_headers()

    def do_DELETE(self):
        if not self.authorized(): return
        assert self.path == "/api/session/ses_fake/form/frm_live"
        FORMS.pop("frm_live")
        self.send_response(204)
        self.end_headers()

    def do_POST(self):
        if not self.authorized():
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        path = urlsplit(self.path).path
        with open(os.path.join(PROJECT, "requests.jsonl"), "a") as requests:
            requests.write(json.dumps({"path": path, "body": body}) + "\n")
        if path == "/api/session/ses_fake/form/frm_saved/reply":
            assert body == {"answer": {"q0": ["macOS", "custom target"]}}
            if os.path.exists(os.path.join(PROJECT, "fail-form-reply")):
                self.send_error(503)
                return
            FORMS.pop("frm_saved")
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session":
            if body.get("model"):
                assert body["model"] == {"providerID": "local", "id": "model/a"}
                assert body["agent"] == "build"
                SESSION["model"] = body["model"]
            self.send_json({"data": SESSION})
        elif path == "/api/session/ses_fake/model":
            if body["model"]["id"] == "missing" or body["model"].get("variant") == "invalid":
                self.send_error(400)
                return
            SESSION["model"] = body["model"]
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/agent":
            assert set(body) == {"agent"}
            if body["agent"] not in ["build", "plan"]:
                self.send_error(400)
                return
            SESSION["agent"] = body["agent"]
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/prompt":
            self.send_json({"data": {"id": "msg_input", "type": "user", "text": body["text"]}})
        elif path == "/api/session/ses_fake/command":
            assert set(body) == {"name", "text", "files"}
            if body["name"] not in ["goal", "team/review"]:
                self.send_error(404)
                return
            self.send_response(204)
            self.end_headers()
        elif path == "/api/session/ses_fake/interrupt":
            self.send_json({"interrupted": True})
        elif path == "/api/session/ses_fake/permission/per_fake/reply":
            with open(os.path.join(PROJECT, "approval.json"), "w") as approval:
                json.dump(body, approval)
            self.send_json({})
        else:
            self.send_error(404)

server = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[-1])), Handler)
if not os.environ.get("OPENCODE_SERVER_PASSWORD"):
    print("server password " + PASSWORD, flush=True)
server.serve_forever()
"##,
        );

        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();
        let questions = QuestionBroker::default();
        let provider = OpenCodeProvider::with_executable(
            Arc::clone(&processes),
            approvals.clone(),
            questions.clone(),
            &executable,
        );
        let events = provider.subscribe();

        let listed = provider.list_sessions(&project_path).unwrap();
        assert_eq!(listed.sessions[0].id, "ses_fake");
        let path = project_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(questions.pending().len(), 4);
        assert!(questions
            .pending()
            .iter()
            .all(|request| request.project_path == path));
        assert_eq!(questions.pending()[0].fields[0]["type"], "multiselect");
        let answer = json!({"q0": ["macOS", "custom target"]})
            .as_object()
            .unwrap()
            .clone();
        fs::write(project_path.join("fail-form-reply"), "").unwrap();
        assert!(questions
            .resolve(
                PROVIDER_NAME,
                &path,
                "ses_fake",
                "frm_saved",
                Some(answer.clone())
            )
            .is_err());
        assert_eq!(questions.pending().len(), 4);
        fs::remove_file(project_path.join("fail-form-reply")).unwrap();
        questions
            .resolve(PROVIDER_NAME, &path, "ses_fake", "frm_saved", Some(answer))
            .unwrap();
        questions
            .resolve(PROVIDER_NAME, &path, "ses_fake", "frm_live", None)
            .unwrap();
        assert_eq!(questions.pending().len(), 2);
        // Rehydrate forms that predate a UI connection, without replaying SSE.
        questions.clear(PROVIDER_NAME, &path, None);
        provider.refresh_questions(&project_path).unwrap();
        assert_eq!(questions.pending().len(), 2);
        let updates = questions.subscribe();
        fs::write(project_path.join("external-form-resolution"), "").unwrap();
        while !updates
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .is_empty()
        {}
        provider.refresh_questions(&project_path).unwrap();
        assert!(questions.pending().is_empty());
        let commands = provider.commands(&project_path).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].name, "goal");
        assert_eq!(
            commands[0].description.as_deref(),
            Some("Set or control an objective")
        );
        assert_eq!(commands[1].name, "team/review");
        assert_eq!(commands[1].description, None);
        let limits = json!({
            "workspace": { "id": "workspace", "usageURL": "https://opencode.ai/workspace/usage" },
            "go": {
                "rolling": { "percent": 42, "limited": false, "resetsAt": 1800000000000_u64 },
                "weekly": { "percent": 20, "limited": false, "resetsAt": 1800100000000_u64 },
                "monthly": { "percent": 100, "limited": true, "resetsAt": 1800200000000_u64 }
            },
            "budgets": []
        });
        fs::write(
            project_path.join("usage.json"),
            serde_json::to_vec(&limits).unwrap(),
        )
        .unwrap();
        assert_eq!(provider.usage_limits(&project_path).unwrap(), limits);
        fs::write(project_path.join("usage.json"), "null").unwrap();
        assert!(provider.usage_limits(&project_path).unwrap().is_null());
        fs::write(project_path.join("command-api-error"), "").unwrap();
        assert!(provider.commands(&project_path).is_err());
        assert!(provider.usage_limits(&project_path).is_err());
        fs::remove_file(project_path.join("command-api-error")).unwrap();
        let choices = provider.choices(&project_path).unwrap();
        assert_eq!(choices.models.len(), 3);
        assert_eq!(choices.models[0].id, "local/model/a");
        assert_eq!(choices.models[1].id, "router/model/a");
        assert_eq!(choices.models[0].context_window, Some(200_000));
        assert_eq!(choices.models[1].context_window, None);
        assert_eq!(choices.models[2].context_window, None);
        assert_eq!(
            choices.models[0].provider.as_ref().unwrap().name,
            "Local gateway"
        );
        assert_eq!(choices.models[1].provider.as_ref().unwrap().id, "router");
        assert_eq!(
            choices.models[1].provider.as_ref().unwrap().name,
            "OpenRouter"
        );
        assert_eq!(choices.models[2].provider.as_ref().unwrap().name, "custom");
        fs::write(project_path.join("no-providers"), "").unwrap();
        let fallback = provider.choices(&project_path).unwrap();
        assert_eq!(fallback.models.len(), 3);
        assert_eq!(fallback.models[0].provider.as_ref().unwrap().name, "local");
        assert_eq!(fallback.models[1].provider.as_ref().unwrap().name, "router");
        assert_eq!(choices.agents.len(), 2);
        assert_eq!(choices.default_model.as_deref(), Some("local/model/a"));
        assert_eq!(
            choices.models[0]
                .efforts
                .iter()
                .map(|choice| choice.id.as_str())
                .collect::<Vec<_>>(),
            ["low", "high"]
        );
        let started = provider
            .start_session_with_options(&project_path, Some(&choices.models[0].id), Some("build"))
            .unwrap();
        assert_eq!(started.session.id, "ses_fake");
        assert_eq!(started.session.model.as_deref(), Some("local/model/a"));
        assert_eq!(started.session.agent.as_deref(), Some("build"));
        let history = provider
            .session_history(&project_path, "ses_fake", None)
            .unwrap();
        assert_eq!(history.data[0]["text"], "Earlier prompt");
        assert_eq!(history.model.as_deref(), Some("local/model/a"));
        assert_eq!(history.agent.as_deref(), Some("build"));
        assert_eq!(
            history.data[1]["content"][1]["state"]["content"][0]["text"],
            "All passed"
        );
        let older = provider
            .session_history(&project_path, "ses_fake", history.next_cursor.as_deref())
            .unwrap();
        assert_eq!(older.data[0]["text"], "Oldest");
        assert_eq!(older.next_cursor, None);
        provider
            .send_prompt(&project_path, "ses_fake", "Continue")
            .unwrap();
        provider
            .send_prompt_with_options(
                &project_path,
                "ses_fake",
                "Switch model",
                Some("msg_selected"),
                Some("other/model/b"),
                Some("high"),
            )
            .unwrap();
        assert_eq!(
            provider
                .resume_session(&project_path, "ses_fake")
                .unwrap()
                .session
                .model
                .as_deref(),
            Some("other/model/b")
        );
        let requests = || {
            fs::read_to_string(project_path.join("requests.jsonl"))
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>()
        };
        let sent = requests();
        let switch = &sent[sent.len() - 2];
        assert_eq!(switch["path"], "/api/session/ses_fake/model");
        assert_eq!(
            switch["body"],
            json!({ "model": { "providerID": "other", "id": "model/b", "variant": "high" } })
        );
        assert_eq!(
            sent.last().unwrap()["body"],
            json!({ "text": "Switch model", "id": "msg_selected" })
        );
        assert!(provider
            .send_prompt_with_options(
                &project_path,
                "ses_fake",
                "Do not send with a rejected model",
                None,
                Some("other/missing"),
                None,
            )
            .is_err());
        assert_eq!(requests().len(), sent.len() + 1);
        assert_eq!(
            requests().last().unwrap()["path"],
            "/api/session/ses_fake/model"
        );
        assert_eq!(
            provider
                .session_history(&project_path, "ses_fake", None)
                .unwrap()
                .effort
                .as_deref(),
            Some("high")
        );
        provider
            .send_prompt_with_options(
                &project_path,
                "ses_fake",
                "Change effort",
                None,
                None,
                Some("low"),
            )
            .unwrap();
        assert_eq!(
            provider
                .resume_session(&project_path, "ses_fake")
                .unwrap()
                .session
                .effort
                .as_deref(),
            Some("low")
        );
        let sent = requests();
        assert_eq!(
            sent[sent.len() - 2]["body"],
            json!({ "model": { "providerID": "other", "id": "model/b", "variant": "low" } })
        );
        assert!(provider
            .send_prompt_with_options(
                &project_path,
                "ses_fake",
                "Rejected effort must not send",
                None,
                None,
                Some("invalid")
            )
            .is_err());
        assert_eq!(requests().len(), sent.len() + 1);
        provider
            .send_prompt_with_options(
                &project_path,
                "ses_fake",
                "Default effort",
                None,
                None,
                Some(""),
            )
            .unwrap();
        let sent = requests();
        assert_eq!(
            sent[sent.len() - 2]["body"],
            json!({ "model": { "providerID": "other", "id": "model/b" } })
        );
        assert_eq!(
            provider
                .resume_session(&project_path, "ses_fake")
                .unwrap()
                .session
                .effort
                .as_deref(),
            Some("")
        );
        provider
            .select_agent(&project_path, "ses_fake", "plan")
            .unwrap();
        assert_eq!(
            requests().last().unwrap(),
            &json!({
                "path": "/api/session/ses_fake/agent", "body": { "agent": "plan" }
            })
        );
        let resumed = provider.resume_session(&project_path, "ses_fake").unwrap();
        assert_eq!(resumed.session.agent.as_deref(), Some("plan"));
        assert_eq!(resumed.session.model.as_deref(), Some("other/model/b"));
        assert_eq!(resumed.session.effort.as_deref(), Some(""));
        assert_eq!(
            provider
                .session_history(&project_path, "ses_fake", None)
                .unwrap()
                .agent
                .as_deref(),
            Some("plan")
        );
        let count = requests().len();
        provider
            .select_agent(&project_path, "ses_fake", "plan")
            .unwrap();
        assert!(matches!(
            provider.select_agent(&project_path, "ses_foreign", "build"),
            Err(OpenCodeError::SessionNotInProject)
        ));
        assert!(provider
            .select_agent(&project_path, "ses_fake", "")
            .is_err());
        assert_eq!(requests().len(), count);
        assert!(provider
            .select_agent(&project_path, "ses_fake", "missing")
            .is_err());
        assert_eq!(
            provider
                .resume_session(&project_path, "ses_fake")
                .unwrap()
                .session
                .agent
                .as_deref(),
            Some("plan")
        );
        let mut command: OpenCodeCommandInput = serde_json::from_value(json!({
            "name": "team/review", "text": "src/main.rs\nKeep Unicode: 日本語 🚀\n",
            "files": [{ "uri": "data:text/plain;base64,aGVsbG8K", "name": "src/main.rs", "description": "Lines 2–3 of src/main.rs" }]
        })).unwrap();
        let count = requests().len();
        assert!(matches!(
            provider.run_command(
                &project_path,
                "ses_foreign",
                &command,
                Some("local/model/a"),
                Some("low")
            ),
            Err(OpenCodeError::SessionNotInProject)
        ));
        assert_eq!(requests().len(), count);
        assert!(provider
            .run_command(
                &project_path,
                "ses_fake",
                &command,
                Some("other/missing"),
                None
            )
            .is_err());
        assert_eq!(requests().len(), count + 1);
        assert_eq!(
            requests().last().unwrap()["path"],
            "/api/session/ses_fake/model"
        );
        provider
            .run_command(
                &project_path,
                "ses_fake",
                &command,
                Some("local/model/a"),
                Some("low"),
            )
            .unwrap();
        let sent = requests();
        assert_eq!(
            sent[sent.len() - 2]["body"],
            json!({ "model": { "providerID": "local", "id": "model/a", "variant": "low" } })
        );
        assert_eq!(
            sent.last().unwrap(),
            &json!({ "path": "/api/session/ses_fake/command", "body": command })
        );
        command.name = "goal".into();
        command.files.clear();
        fs::write(project_path.join("busy"), "").unwrap();
        let checkout = project_path.canonicalize().unwrap();
        assert!(provider.ensure_checkout_idle(&checkout).is_err());
        for text in [
            "Finish the task\nThen summarize.",
            "pause",
            "resume",
            "clear",
            "",
        ] {
            command.text = text.into();
            let count = requests().len();
            provider
                .run_command(&project_path, "ses_fake", &command, None, None)
                .unwrap();
            assert_eq!(requests().len(), count + 1);
            assert_eq!(
                requests().last().unwrap()["body"],
                json!({ "name": "goal", "text": text, "files": [] })
            );
        }
        fs::remove_file(project_path.join("busy")).unwrap();
        provider.ensure_checkout_idle(&checkout).unwrap();
        command.name = "unknown".into();
        assert!(provider
            .run_command(&project_path, "ses_fake", &command, None, None)
            .is_err());
        command.name = " ".into();
        let count = requests().len();
        assert!(provider
            .run_command(&project_path, "ses_fake", &command, None, None)
            .is_err());
        assert_eq!(requests().len(), count);
        provider.cancel_turn(&project_path, "ses_fake").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut received_text = false;
        let mut received_permission = false;
        while Instant::now() < deadline && !(received_text && received_permission) {
            let streamed = events.recv_timeout(Duration::from_millis(100)).unwrap();
            if streamed.event_type == "session.text.delta" {
                received_text = streamed.data["delta"] == "hello";
            }
            if streamed.event_type == "permission.asked" {
                received_permission = true;
            }
        }
        assert!(received_text);
        assert!(received_permission);
        let prompt_deadline = Instant::now() + Duration::from_secs(3);
        while approvals.pending().is_empty() && Instant::now() < prompt_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let prompt = approvals
            .pending()
            .pop()
            .expect("permission prompt is pending");
        assert_eq!(prompt.provider, PROVIDER_NAME);
        assert_eq!(
            prompt.project_path,
            project_path.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(prompt.session_id.as_deref(), Some("ses_fake"));
        approvals
            .resolve(
                PROVIDER_NAME,
                &prompt.project_path,
                &prompt.request_id,
                ApprovalDecision::AllowOnce,
            )
            .unwrap();
        let approval_path = project_path.join("approval.json");
        let reply_deadline = Instant::now() + Duration::from_secs(3);
        while !approval_path.exists() && Instant::now() < reply_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let reply: Value = serde_json::from_slice(&fs::read(approval_path).unwrap()).unwrap();
        assert_eq!(reply["decision"], "once");

        let details = provider.session_details(&project_path, "ses_fake").unwrap();
        assert_eq!(details.branch, None);
        assert!(!details.branch_is_current);
        assert_eq!(
            details.changes,
            Some(LineChanges {
                additions: 15,
                deletions: 7
            })
        );
        fs::write(project_path.join("no-vcs"), "").unwrap();
        let details = provider.session_details(&project_path, "ses_fake").unwrap();
        assert_eq!(details.branch, None);
        assert_eq!(details.changes, None);
        assert!(matches!(
            provider.session_details(&project_path, "ses_foreign"),
            Err(OpenCodeError::SessionNotInProject)
        ));
        for archived in [true, false] {
            assert!(matches!(
                provider.set_session_archived(&project_path, "ses_foreign", archived),
                Err(OpenCodeError::SessionNotInProject)
            ));
        }
        fs::write(project_path.join("busy"), "").unwrap();
        assert!(provider
            .set_session_archived(&project_path, "ses_fake", true)
            .is_err());
        fs::remove_file(project_path.join("busy")).unwrap();
        fs::write(project_path.join("archive-error"), "").unwrap();
        assert!(provider
            .set_session_archived(&project_path, "ses_fake", true)
            .is_err());
        assert_eq!(
            provider
                .list_sessions(&project_path)
                .unwrap()
                .sessions
                .len(),
            1
        );
        fs::remove_file(project_path.join("archive-error")).unwrap();
        provider
            .set_session_archived(&project_path, "ses_fake", true)
            .unwrap();
        assert!(provider
            .list_sessions(&project_path)
            .unwrap()
            .sessions
            .is_empty());
        provider
            .set_session_archived(&project_path, "ses_fake", false)
            .unwrap();
        assert_eq!(
            provider.list_sessions(&project_path).unwrap().sessions[0].id,
            "ses_fake"
        );

        let process_id = provider
            .inner
            .lock_servers()
            .get(&Project::open(&project_path).unwrap().path().to_path_buf())
            .unwrap()
            .process_id
            .unwrap();
        provider.release_checkout(&checkout);
        assert!(provider.inner.lock_servers().is_empty());
        provider.shutdown();
        assert!(matches!(
            processes.terminate(process_id),
            Err(crate::processes::ProcessError::UnknownProcess)
        ));
    }

    #[test]
    fn reserve_local_port_returns_a_loopback_port_that_can_be_rebound() {
        let port = reserve_local_port().unwrap();
        assert_ne!(port, 0);
    }

    #[test]
    fn registered_service_streams_external_work_and_survives_provider_shutdown() {
        if Command::new("python3").arg("--version").output().is_err() {
            return;
        }
        struct ExternalServer(std::process::Child);
        impl Drop for ExternalServer {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        for password in [None, Some("shared-secret")] {
            let directory = TestDirectory::new();
            let script = directory.0.join("service.py");
            fs::write(&script, r#"
import base64, json, os, queue, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

events = queue.Queue()
subscribed = False
password = os.environ.get('TEST_PASSWORD')
session = {'id': 'ses_shared', 'title': 'External work', 'location': {'directory': os.getcwd()}, 'time': {'idle': 1}}
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args): pass
    def authorized(self):
        expected = 'Basic ' + base64.b64encode(('opencode:' + password).encode()).decode() if password else None
        if self.headers.get('Authorization') == expected: return True
        self.send_error(401)
        return False
    def reply(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        global subscribed
        if not self.authorized(): return
        path = urlsplit(self.path).path
        if path == '/api/info':
            self.reply({'pid': os.getpid(), 'version': '2.2.1'})
        elif path == '/api/event':
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.end_headers()
            # Headers alone are not a subscription acknowledgement.
            time.sleep(0.1)
            subscribed = True
            self.wfile.write(b'data: {"type":"server.connected","data":{}}\n\n')
            self.wfile.flush()
            try:
                while True:
                    try: frame = ('data: ' + json.dumps(events.get(timeout=0.1)) + '\n\n').encode()
                    except queue.Empty: frame = b': keepalive\n\n'
                    self.wfile.write(frame)
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError): pass
        elif path == '/api/session':
            assert subscribed
            self.reply({'data': [session]})
        elif path == '/api/session/active':
            if os.path.exists('activity-error'): self.send_error(503)
            elif os.path.exists('activity-invalid'): self.reply({'data': []})
            else: self.reply({'data': {} if os.path.exists('idle') else {'ses_shared': {'type': 'running'}}})
        elif path == '/api/session/ses_shared': self.reply({'data': session})
        elif path == '/api/session/ses_shared/message': self.reply({'data': [], 'cursor': {'next': None}})
        else: self.send_error(404)
    def do_POST(self):
        if not self.authorized(): return
        events.put({'type': 'session.text.delta', 'location': {'directory': '/other-project'}, 'data': {'sessionID': 'ses_shared', 'delta': 'Wrong project'}})
        events.put({'type': 'session.reasoning.delta', 'data': {'sessionID': 'ses_shared', 'assistantMessageID': 'msg_external', 'ordinal': 0, 'delta': 'External thinking'}})
        self.reply({})
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
print('http://127.0.0.1:' + str(server.server_port), flush=True)
server.serve_forever()
"#).unwrap();
            let mut command = Command::new("python3");
            command
                .arg(script)
                .current_dir(&directory.0)
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            if let Some(password) = password {
                command.env("TEST_PASSWORD", password);
            } else {
                command.env_remove("TEST_PASSWORD");
            }
            let mut external = ExternalServer(command.spawn().unwrap());
            let mut url = String::new();
            BufReader::new(external.0.stdout.take().unwrap())
                .read_line(&mut url)
                .unwrap();
            let url = url.trim();
            let registration = directory.0.join("service.json");
            let mut record = json!({ "url": url, "pid": external.0.id(), "version": "2.2.1", "password": password });
            // Stale registrations must not attach to a different process/version.
            record["pid"] = json!(external.0.id() + 1);
            fs::write(&registration, record.to_string()).unwrap();
            assert!(discover_service(&registration).is_none());
            record["pid"] = json!(external.0.id());
            record["version"] = json!("2.0.0");
            fs::write(&registration, record.to_string()).unwrap();
            assert!(discover_service(&registration).is_none());
            record["version"] = json!("2.2.1");
            fs::write(&registration, record.to_string()).unwrap();

            let mut provider = OpenCodeProvider::new(
                Arc::new(ProcessRegistry::default()),
                ApprovalBroker::default(),
                QuestionBroker::default(),
            );
            Arc::get_mut(&mut provider.inner).unwrap().service_file = Some(registration);
            let events = provider.subscribe();
            let sessions = provider.list_sessions(&directory.0).unwrap();
            assert!(sessions.sessions[0].is_working);
            assert_eq!(
                provider
                    .session_history(&directory.0, "ses_shared", None)
                    .unwrap()
                    .is_working,
                Some(true)
            );
            let server = Arc::clone(provider.inner.lock_servers().values().next().unwrap());
            assert!(server.process_id.is_none());
            let checkout = directory.0.canonicalize().unwrap();
            assert!(provider.ensure_checkout_idle(&checkout).is_err());
            // Trigger the service independently of Caelum's prompt endpoint.
            server
                .request_json(Method::POST, "/external", &[], None)
                .unwrap();
            let event = events.recv_timeout(Duration::from_secs(3)).unwrap();
            assert_eq!(event.event_type, "session.reasoning.delta");
            assert_eq!(event.data["delta"], "External thinking");
            assert!(events.try_recv().is_err());
            // Repeated history polls and resume must agree with the runtime,
            // even though persisted session metadata has only an old idle time.
            for _ in 0..3 {
                assert!(
                    provider
                        .resume_session(&directory.0, "ses_shared")
                        .unwrap()
                        .session
                        .is_working
                );
                assert_eq!(
                    provider
                        .session_history(&directory.0, "ses_shared", None)
                        .unwrap()
                        .is_working,
                    Some(true)
                );
            }
            // An unavailable or malformed activity response must not report idle.
            fs::write(directory.0.join("activity-error"), "").unwrap();
            assert!(matches!(
                provider.session_history(&directory.0, "ses_shared", None),
                Err(OpenCodeError::HttpStatus { status: 503, .. })
            ));
            fs::remove_file(directory.0.join("activity-error")).unwrap();
            fs::write(directory.0.join("activity-invalid"), "").unwrap();
            assert!(matches!(
                provider.list_sessions(&directory.0),
                Err(OpenCodeError::Protocol(_))
            ));
            fs::remove_file(directory.0.join("activity-invalid")).unwrap();
            fs::write(directory.0.join("idle"), "").unwrap();
            assert!(!provider.list_sessions(&directory.0).unwrap().sessions[0].is_working);
            assert_eq!(
                provider
                    .session_history(&directory.0, "ses_shared", None)
                    .unwrap()
                    .is_working,
                Some(false)
            );
            provider.ensure_checkout_idle(&checkout).unwrap();
            provider.release_checkout(&checkout);
            assert!(provider.inner.lock_servers().is_empty());
            assert!(!server.alive.load(Ordering::Acquire));
            provider.shutdown();
            assert!(external.0.try_wait().unwrap().is_none());
            assert!(server
                .request_json(Method::GET, "/api/info", &[], None)
                .is_ok());
        }
    }
}

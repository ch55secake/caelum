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
    ApprovalDecision, OpenCodeEvent, OpenCodeSession, OpenCodeSessionInfo, OpenCodeSessionList,
    PermissionRequest, ProviderChoice, ProviderChoices, SessionHistoryPage,
};
use reqwest::{blocking::Client, Method, StatusCode, Url};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    approvals::ApprovalBroker,
    processes::{ProcessId, ProcessRegistry},
    projects::{Project, ProjectError},
};

const PROVIDER_NAME: &str = "opencode";
const SERVER_PASSWORD_ENV: &str = "OPENCODE_SERVER_PASSWORD";
const SERVER_START_TIMEOUT: Duration = Duration::from_secs(12);
const SERVER_POLL_INTERVAL: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SESSION_LIMIT: &str = "100";

type EventSubscribers = Arc<Mutex<Vec<Sender<OpenCodeEvent>>>>;

/// Manages one official OpenCode HTTP server for each selected project.
/// Servers bind only to loopback and use OpenCode's `/api` and `/api/event`
/// endpoints.
#[derive(Clone)]
pub struct OpenCodeProvider {
    inner: Arc<ProviderInner>,
}

struct ProviderInner {
    executable: Option<PathBuf>,
    processes: Arc<ProcessRegistry>,
    approvals: ApprovalBroker,
    servers: Mutex<HashMap<PathBuf, Arc<OpenCodeServer>>>,
    subscribers: EventSubscribers,
    closed: AtomicBool,
}

struct OpenCodeServer {
    process_id: ProcessId,
    base_url: String,
    client: Client,
    event_client: Client,
    password: String,
    alive: Arc<AtomicBool>,
    session_ids: Mutex<HashSet<String>>,
    pending_permission_ids: Arc<Mutex<HashSet<String>>>,
}

impl OpenCodeProvider {
    pub fn new(processes: Arc<ProcessRegistry>, approvals: ApprovalBroker) -> Self {
        Self::with_optional_executable(processes, approvals, None)
    }

    /// Construct a provider with an explicit OpenCode executable. This is
    /// useful for non-standard installations and lifecycle tests.
    pub fn with_executable(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        executable: impl Into<PathBuf>,
    ) -> Self {
        Self::with_optional_executable(processes, approvals, Some(executable.into()))
    }

    fn with_optional_executable(
        processes: Arc<ProcessRegistry>,
        approvals: ApprovalBroker,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(ProviderInner {
                executable,
                processes,
                approvals,
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

        let mut sessions = Vec::with_capacity(values.len());
        for value in values {
            if session_directory(value)
                .is_some_and(|directory| directory_matches(directory, project.path()))
            {
                if let Some(session) = session_info(value) {
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
        let models = models
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OpenCodeError::Protocol("model list has no data array".into()))?
            .iter()
            .filter(|model| model.get("enabled").and_then(Value::as_bool) == Some(true))
            .filter_map(|model| {
                Some(ProviderChoice {
                    id: format!(
                        "{}/{}",
                        model.get("providerID")?.as_str()?,
                        model.get("id")?.as_str()?
                    ),
                    name: model.get("name")?.as_str()?.to_owned(),
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
        Ok(ProviderChoices { models, agents })
    }

    pub fn start_session_with_options(
        &self,
        project_path: impl AsRef<Path>,
        model: Option<&str>,
        agent: Option<&str>,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let model = model
            .map(|model| {
                model
                    .split_once('/')
                    .map(|(provider, id)| json!({ "providerID": provider, "id": id }))
                    .ok_or_else(|| {
                        OpenCodeError::Protocol("model must include its provider".into())
                    })
            })
            .transpose()?;
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
        self.session(&server, &project, session_id)?;
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

    pub fn send_prompt_with_id(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        prompt: &str,
        message_id: Option<&str>,
    ) -> Result<(), OpenCodeError> {
        if prompt.trim().is_empty() {
            return Err(OpenCodeError::EmptyPrompt);
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let path = format!("/api/session/{session_id}/prompt");
        server.request_json(
            Method::POST,
            &path,
            &[],
            Some(json!({ "text": prompt, "id": message_id })),
        )?;
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
        Ok(())
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
        for (_, server) in servers {
            server.alive.store(false, Ordering::Release);
            let _ = self.inner.processes.terminate(server.process_id);
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
        let session = session_info(value)
            .ok_or_else(|| OpenCodeError::Protocol("session response has no session ID".into()))?;
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
            let _ = self.inner.processes.terminate(dead.process_id);
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
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                let _ = self.inner.processes.terminate(process_id);
                OpenCodeError::Request(error.to_string())
            })?;
        let event_client = Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                let _ = self.inner.processes.terminate(process_id);
                OpenCodeError::Request(error.to_string())
            })?;
        let server = Arc::new(OpenCodeServer {
            process_id,
            base_url: format!("http://127.0.0.1:{port}"),
            client,
            event_client,
            password,
            alive: Arc::new(AtomicBool::new(true)),
            session_ids: Mutex::new(HashSet::new()),
            pending_permission_ids: Arc::new(Mutex::new(HashSet::new())),
        });

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

        if self.inner.closed.load(Ordering::Acquire) {
            let _ = self.inner.processes.terminate(process_id);
            return Err(OpenCodeError::Closed);
        }
        spawn_event_listener(
            Arc::clone(&server),
            project.path().to_path_buf(),
            self.inner.approvals.clone(),
            Arc::clone(&self.inner.subscribers),
        );
        servers.insert(project.path().to_path_buf(), Arc::clone(&server));
        Ok(server)
    }
}

impl OpenCodeServer {
    fn request_json(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, OpenCodeError> {
        let url = api_url(&self.base_url, path, query)?;
        let mut request = self
            .client
            .request(method, url)
            .basic_auth("opencode", Some(&self.password));
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

    fn owns_session(&self, session_id: &str) -> bool {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(session_id)
    }
}

fn spawn_event_listener(
    server: Arc<OpenCodeServer>,
    project_path: PathBuf,
    approvals: ApprovalBroker,
    subscribers: EventSubscribers,
) {
    thread::spawn(move || {
        let url = match api_url(&server.base_url, "/api/event", &[]) {
            Ok(url) => url,
            Err(_) => return,
        };
        let request = server
            .event_client
            .get(url)
            .basic_auth("opencode", Some(&server.password));
        let result = request.send();
        let Ok(response) = result else {
            publish_disconnected(&server, &project_path, &approvals, &subscribers);
            return;
        };
        if !response.status().is_success() {
            publish_disconnected(&server, &project_path, &approvals, &subscribers);
            return;
        }

        let mut reader = BufReader::new(response);
        let _ = read_sse_frames(&mut reader, |payload| {
            let Ok(event) = parse_event(&payload, &project_path) else {
                return;
            };
            if !event_belongs_to_project(
                event.location.as_ref(),
                &event.data,
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
            publish_event(&subscribers, event);
        });
        publish_disconnected(&server, &project_path, &approvals, &subscribers);
    });
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
    subscribers: &EventSubscribers,
) {
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

fn read_sse_frames<R, F>(reader: &mut R, mut on_data: F) -> io::Result<()>
where
    R: BufRead,
    F: FnMut(String),
{
    let mut data = Vec::new();
    loop {
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
        data: value.get("data").cloned().unwrap_or(Value::Null),
        location: value.get("location").cloned(),
    })
}

fn session_info(value: &Value) -> Option<OpenCodeSession> {
    Some(OpenCodeSession {
        is_working: value
            .pointer("/time/busy")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > value
                .pointer("/time/idle")
                .and_then(Value::as_u64)
                .unwrap_or(0),
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
        for (_, server) in std::mem::take(servers) {
            server.alive.store(false, Ordering::Release);
            let _ = self.processes.terminate(server.process_id);
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

        read_sse_frames(&mut reader, |frame| frames.push(frame)).unwrap();

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
            r#"{"id":"evt_1","type":"session.text.delta","location":{"directory":"/workspace"},"data":{"sessionID":"ses_1","delta":"hi"}}"#,
            Path::new("/workspace"),
        )
        .unwrap();

        assert_eq!(event.event_type, "session.text.delta");
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
        let provider = OpenCodeProvider::new(Arc::clone(&processes), approvals);
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let process_id = processes.register(child).unwrap();
        let client = Client::builder().build().unwrap();
        let project_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let server = Arc::new(OpenCodeServer {
            process_id,
            base_url: "http://127.0.0.1:1".to_owned(),
            client: client.clone(),
            event_client: client,
            password: "unused-test-password".to_owned(),
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
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let project_path = directory.0.join("project");
        fs::create_dir_all(&project_path).unwrap();
        let executable = directory.0.join("fake-opencode");
        fs::write(
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
SESSION = {"id": "ses_fake", "title": "Fake session", "time": {"updated": 123}, "location": {"directory": PROJECT}}

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
                {"id": "evt_fake", "type": "session.text.delta", "location": {"directory": PROJECT}, "data": {"sessionID": "ses_fake", "assistantMessageID": "msg_fake", "delta": "hello"}},
                {"id": "evt_permission", "type": "permission.asked", "location": {"directory": PROJECT}, "data": {"id": "per_fake", "sessionID": "ses_fake", "action": "bash", "resources": ["echo hello"], "message": "Run a command?"}}
            ]
            for event in events:
                self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
            self.wfile.flush()
            try:
                while True:
                    time.sleep(1)
                    self.wfile.write(b": keepalive\n\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                return
        elif path == "/api/session":
            self.send_json({"data": [SESSION], "cursor": {"previous": None, "next": None}})
        elif path == "/api/session/ses_fake":
            self.send_json({"data": SESSION})
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
            self.send_json({"data": [{"id": "model/a", "providerID": "local", "name": "Model A", "enabled": True}, {"id": "disabled", "providerID": "local", "name": "Hidden", "enabled": False}]})
        elif path == "/api/agent":
            self.send_json({"data": [{"id": "build", "name": "Build", "mode": "primary", "hidden": False}, {"id": "child", "name": "Child", "mode": "subagent", "hidden": False}]})
        else:
            self.send_error(404)

    def do_POST(self):
        if not self.authorized():
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length) or b"{}")
        path = urlsplit(self.path).path
        if path == "/api/session":
            if body.get("model"):
                assert body["model"] == {"providerID": "local", "id": "model/a"}
                assert body["agent"] == "build"
            self.send_json({"data": SESSION})
        elif path == "/api/session/ses_fake/prompt":
            self.send_json({"data": {"id": "msg_input", "type": "user", "text": body["text"]}})
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
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let processes = Arc::new(ProcessRegistry::default());
        let approvals = ApprovalBroker::default();
        let provider = OpenCodeProvider::with_executable(
            Arc::clone(&processes),
            approvals.clone(),
            &executable,
        );
        let events = provider.subscribe();

        let listed = provider.list_sessions(&project_path).unwrap();
        assert_eq!(listed.sessions[0].id, "ses_fake");
        let choices = provider.choices(&project_path).unwrap();
        assert_eq!(choices.models.len(), 1);
        assert_eq!(choices.agents.len(), 1);
        let started = provider
            .start_session_with_options(&project_path, Some(&choices.models[0].id), Some("build"))
            .unwrap();
        assert_eq!(started.session.id, "ses_fake");
        let history = provider
            .session_history(&project_path, "ses_fake", None)
            .unwrap();
        assert_eq!(history.data[0]["text"], "Earlier prompt");
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

        let process_id = provider
            .inner
            .lock_servers()
            .get(&Project::open(&project_path).unwrap().path().to_path_buf())
            .unwrap()
            .process_id;
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
}

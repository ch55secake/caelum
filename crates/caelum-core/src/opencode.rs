use std::{
    collections::{HashMap, HashSet},
    io,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};

use caelum_protocol::{
    OpenCodeCommandInfo, OpenCodeCommandInput, OpenCodeEvent, OpenCodeSession, ProviderChoice,
    ProviderChoices, ProviderModelChoice,
};
use reqwest::{blocking::Client, Method};
use serde_json::{json, Value};
use thiserror::Error;

mod events;
mod generate;
mod permissions;
mod questions;
mod server;
mod sessions;
mod shells;
mod stats;
#[cfg(test)]
mod tests;
mod transport;

use permissions::deny_pending_approvals;
use questions::sync_questions;
use server::service_file;
use server::ServerSlot;
use transport::api_url;

use crate::{
    approvals::ApprovalBroker,
    processes::{ProcessId, ProcessRegistry},
    projects::{Project, ProjectError},
    questions::QuestionBroker,
};

const PROVIDER_NAME: &str = "opencode";
const SERVER_START_TIMEOUT: Duration = Duration::from_secs(12);
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
    servers: Mutex<HashMap<PathBuf, Arc<ServerSlot>>>,
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
        let (prompt, images) = crate::application::provider_prompt(prompt);
        let mut body = json!({"text":prompt,"id":message_id});
        if !images.is_empty() {
            body["files"] = json!(images
                .iter()
                .map(|a| json!({"uri":a.content,"name":a.path}))
                .collect::<Vec<_>>());
        }
        server.request_json(Method::POST, &path, &[], Some(body))?;
        Ok(())
    }

    pub fn enqueue_prompt(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        message_id: &str,
        prompt: &str,
    ) -> Result<(), OpenCodeError> {
        if prompt.trim().is_empty() {
            return Err(OpenCodeError::EmptyPrompt);
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let (text, images) = crate::application::provider_prompt(prompt);
        let mut body = json!({ "id": message_id, "text": text, "delivery": "queue" });
        if !images.is_empty() {
            body["files"] = json!(images
                .iter()
                .map(|a| json!({"uri": a.content,"name":a.path}))
                .collect::<Vec<_>>());
        }
        server.request_json(
            Method::POST,
            &format!("/api/session/{session_id}/prompt"),
            &[],
            Some(body),
        )?;
        Ok(())
    }

    pub fn inbox(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<Vec<Value>, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let response = server.request_json(
            Method::GET,
            &format!("/api/session/{session_id}/inbox"),
            &[],
            None,
        )?;
        response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| OpenCodeError::Protocol("Inbox response has no data array".into()))
    }

    pub fn cancel_queued_prompt(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), OpenCodeError> {
        if !message_id.starts_with("msg")
            || !message_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(OpenCodeError::Protocol("Invalid inbox message ID".into()));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        server.request_json(
            Method::DELETE,
            &format!("/api/session/{session_id}/inbox/{message_id}"),
            &[],
            None,
        )?;
        Ok(())
    }

    pub fn read_image(
        &self,
        project_path: impl AsRef<Path>,
        hash: &str,
    ) -> Result<String, OpenCodeError> {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(OpenCodeError::Protocol("Invalid media hash".into()));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let url = api_url(
            &server.base_url,
            &format!("/api/experimental/media/{hash}"),
            &[],
        )?;
        let response = server
            .authenticate(server.client.get(url))
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| OpenCodeError::Request(error.to_string()))?;
        crate::images::data_url(response).map_err(OpenCodeError::Protocol)
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
            .filter(|(path, _)| path.starts_with(checkout))
            .map(|(path, server)| (path.clone(), Arc::clone(server)))
            .collect();
        for (path, slot) in servers {
            let Some(server) = slot.current() else {
                continue;
            };
            if Self::list_sessions_on(&Project::open(&path)?, &server, false)?
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
        let removed = {
            let mut servers = self.inner.lock_servers();
            let paths: Vec<_> = servers
                .keys()
                .filter(|path| path.starts_with(checkout))
                .cloned()
                .collect();
            paths
                .into_iter()
                .filter_map(|path| {
                    let server = servers.remove(&path)?.retire();
                    // Invalidate recovery and clear forms before a replacement
                    // slot can be registered. No provider I/O holds the map.
                    self.inner
                        .questions
                        .clear(PROVIDER_NAME, &path.to_string_lossy(), None);
                    server
                })
                .collect::<Vec<_>>()
        };
        for server in removed {
            server.close(&self.inner.processes);
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

fn reserve_local_port() -> Result<u16, OpenCodeError> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(OpenCodeError::PortAssignment)?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(OpenCodeError::PortAssignment)
}

fn find_opencode_executable() -> Result<PathBuf, OpenCodeError> {
    crate::executable::find("opencode").ok_or(OpenCodeError::NotInstalled)
}

impl ProviderInner {
    fn lock_servers(&self) -> MutexGuard<'_, HashMap<PathBuf, Arc<ServerSlot>>> {
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
    #[error("OpenCode CLI was not found. Install OpenCode, then refresh provider status.")]
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

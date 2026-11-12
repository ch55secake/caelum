use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use reqwest::{blocking::Client, Method, Url};
use serde_json::Value;

use super::{
    events::spawn_event_listener,
    find_opencode_executable,
    questions::sync_questions,
    reserve_local_port,
    transport::{api_url, capture_server_password, decode_json_response},
    OpenCodeError, OpenCodeProvider, OpenCodeServer, SERVER_START_TIMEOUT,
};
use crate::{
    processes::{ProcessId, ProcessRegistry},
    projects::Project,
};

pub(super) const SERVER_PASSWORD_ENV: &str = "OPENCODE_SERVER_PASSWORD";
const SERVER_POLL_INTERVAL: Duration = Duration::from_millis(100);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

impl OpenCodeProvider {
    pub(super) fn server(&self, project: &Project) -> Result<Arc<OpenCodeServer>, OpenCodeError> {
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
            Some(path) if crate::executable::executable(path) => path.to_path_buf(),
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
            .env("PATH", crate::executable::path(&executable))
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

impl OpenCodeServer {
    pub(super) fn new(
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

    pub(super) fn authenticate(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match &self.password {
            Some(password) => request.basic_auth("opencode", Some(password)),
            None => request,
        }
    }

    pub(super) fn close(&self, processes: &ProcessRegistry) {
        self.alive.store(false, Ordering::Release);
        if let Some(id) = self.process_id {
            let _ = processes.terminate(id);
        }
    }

    pub(super) fn request_json(
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

    pub(super) fn remember_session(&self, session_id: &str) {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(session_id.to_owned());
    }

    pub(super) fn forget_session(&self, session_id: &str) {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(session_id);
    }

    pub(super) fn active_sessions(&self) -> Result<HashSet<String>, OpenCodeError> {
        // Activity belongs to the runtime drain, not persisted busy/idle times.
        // A running session can have no busy timestamp (or a previous idle time).
        let response = self.request_json(Method::GET, "/api/session/active", &[], None)?;
        let active = response
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| OpenCodeError::Protocol("active sessions has no data map".into()))?;
        Ok(active.keys().cloned().collect())
    }

    pub(super) fn owns_session(&self, session_id: &str) -> bool {
        self.session_ids
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(session_id)
    }
}

pub(super) fn service_file() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .map(|state| state.join("opencode/service.json"))
}

pub(super) fn discover_service(file: &Path) -> Option<Arc<OpenCodeServer>> {
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

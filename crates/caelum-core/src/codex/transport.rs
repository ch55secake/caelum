use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    thread,
};

use caelum_protocol::{ApprovalDecision, CodexEvent};
use serde_json::{json, Value};

use super::requests::{handle_server_request, request_question};
use super::{
    id_key, project_key, request_id_string, CodexConnection, CodexError, EventSubscribers,
    ReaderContext, PROVIDER_NAME, REQUEST_TIMEOUT,
};

impl CodexConnection {
    pub(super) fn queue_turn(&self, thread_id: &str) {
        self.pending_turns
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(thread_id.to_owned());
    }

    pub(super) fn complete_turn_start(&self, thread_id: &str, result: &Value) {
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

    pub(super) fn remove_pending_turn(&self, thread_id: &str) {
        let mut pending = self.pending_turns.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(index) = pending
            .iter()
            .position(|pending_id| pending_id == thread_id)
        {
            pending.remove(index);
        }
    }

    pub(super) fn remember_thread(&self, thread_id: &str) {
        self.thread_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(thread_id.to_owned());
    }

    pub(super) fn owns_thread(&self, thread_id: &str) -> bool {
        self.thread_ids
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(thread_id)
    }

    pub(super) fn request(&self, method: &str, params: Value) -> Result<Value, CodexError> {
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

    pub(super) fn notify(&self, method: &str, params: Value) -> Result<(), CodexError> {
        write_message(&self.writer, &json!({ "method": method, "params": params }))
            .map_err(CodexError::Transport)
    }
}

pub(super) fn spawn_reader(stdout: std::process::ChildStdout, context: ReaderContext) {
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

pub(super) fn write_message(
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

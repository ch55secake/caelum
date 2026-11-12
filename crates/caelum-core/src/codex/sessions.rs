use super::{CodexConnection, CodexError, CodexProvider, PROVIDER_NAME};
use crate::projects::Project;
use caelum_protocol::{
    CodexSessionInfo, CodexSessionList, CodexThreadInfo, LineChanges, ProviderChoice,
    ProviderChoices, ProviderModelChoice, SessionDetails, SessionHistoryPage,
};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

impl CodexProvider {
    /// Return recent Codex threads whose working directory is this project.
    pub fn list_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<CodexSessionList, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        Self::list_sessions_on(&project, &connection, false)
    }

    /// Return archived Codex threads whose working directory is this project.
    pub fn list_archived_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<CodexSessionList, CodexError> {
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        Self::list_sessions_on(&project, &connection, true)
    }

    pub(super) fn list_sessions_on(
        project: &Project,
        connection: &CodexConnection,
        archived: bool,
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
                    "archived": archived,
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
        let (prompt, images) = crate::application::provider_prompt(prompt);
        let mut input = Vec::new();
        if !prompt.is_empty() {
            input.push(json!({"type":"text","text":prompt}));
        }
        input.extend(
            images
                .iter()
                .map(|a| json!({"type":"image","url":a.content})),
        );
        match connection.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "model": model,
                "effort": effort,
                "input": input
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
            branches: Vec::new(),
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

    pub fn delete_session(
        &self,
        project_path: impl AsRef<Path>,
        thread_id: &str,
    ) -> Result<(), CodexError> {
        if thread_id.trim().is_empty() {
            return Err(CodexError::InvalidThreadId);
        }
        let project = Project::open(project_path)?;
        let connection = self.connection(&project)?;
        let response = connection.request("thread/read", json!({ "threadId": thread_id }))?;
        let cwd = response
            .pointer("/thread/cwd")
            .and_then(Value::as_str)
            .ok_or(CodexError::ThreadNotInProject)?;
        if Path::new(cwd).canonicalize().ok().as_deref() != Some(project.path()) {
            return Err(CodexError::ThreadNotInProject);
        }
        if response
            .pointer("/thread/status/type")
            .and_then(Value::as_str)
            == Some("active")
        {
            return Err(CodexError::Protocol(
                "stop the running turn before deleting this session".into(),
            ));
        }
        if !Self::list_sessions_on(&project, &connection, true)?
            .threads
            .iter()
            .any(|thread| thread.id == thread_id)
        {
            return Err(CodexError::Protocol(
                "only archived Codex sessions can be deleted".into(),
            ));
        }
        connection.request("thread/delete", json!({ "threadId": thread_id }))?;
        self.inner.questions.clear(
            PROVIDER_NAME,
            &project.path().to_string_lossy(),
            Some(thread_id),
        );
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

pub(super) fn file_change_lines(change: &Value) -> LineChanges {
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

use super::{
    model_id, model_ref, valid_session_id, OpenCodeError, OpenCodeProvider, OpenCodeServer,
    PROVIDER_NAME, SESSION_LIMIT,
};
use crate::projects::Project;
use caelum_protocol::{
    LineChanges, OpenCodeSession, OpenCodeSessionInfo, OpenCodeSessionList, SessionDetails,
    SessionHistoryPage,
};
use reqwest::Method;
use serde_json::{json, Value};
use std::path::Path;

impl OpenCodeProvider {
    pub(crate) fn fork_session(
        &self,
        path: &str,
        source: &str,
        message: &str,
        before: bool,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        if !valid_message_id(message) {
            return Err(OpenCodeError::Protocol(
                "Invalid OpenCode fork boundary".into(),
            ));
        }
        let project = self.project(path)?;
        let server = self.server(&project)?;
        let parent = self.session(&server, &project, source)?;
        if parent.is_working || parent.archived {
            return Err(OpenCodeError::Protocol(
                "Fork an idle, unarchived session.".into(),
            ));
        }
        let route = format!("/api/session/{source}");
        let body = if before {
            json!({"before":message})
        } else {
            let latest = server.request_json(
                Method::GET,
                &format!("{route}/message"),
                &[("limit", "1".into()), ("order", "desc".into())],
                None,
            )?;
            if latest.pointer("/data/0/id").and_then(Value::as_str) != Some(message) {
                return Err(OpenCodeError::Protocol(
                    "The conversation changed. Reload the fork picker to choose the new endpoint."
                        .into(),
                ));
            }
            json!({})
        };
        let response =
            server.request_json(Method::POST, &format!("{route}/fork"), &[], Some(body))?;
        let value = &response["data"];
        let boundary_type = if before { "before" } else { "through" };
        if !session_directory(value).is_some_and(|dir| project.matches(dir))
            || value.pointer("/fork/sessionID").and_then(Value::as_str) != Some(source)
            || value.pointer("/fork/boundary/type").and_then(Value::as_str) != Some(boundary_type)
            || value
                .pointer("/fork/boundary/messageID")
                .and_then(Value::as_str)
                != Some(message)
        {
            return Err(OpenCodeError::Protocol("OpenCode returned a different fork boundary or checkout. Refresh sessions before retrying.".into()));
        }
        let session = session_info(value)
            .filter(|s| valid_session_id(&s.id) && s.id != source)
            .ok_or_else(|| {
                OpenCodeError::Protocol("OpenCode returned an invalid fork identity.".into())
            })?;
        server.remember_session(&session.id);
        Ok(OpenCodeSessionInfo {
            project: project.info(),
            session,
        })
    }

    pub fn list_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        Self::list_sessions_on(&project, &server, false)
    }

    /// Return archived OpenCode sessions for this project.
    pub fn list_archived_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        Self::list_sessions_on(&project, &server, true)
    }

    pub(super) fn list_sessions_on(
        project: &Project,
        server: &OpenCodeServer,
        archived: bool,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let query = [
            ("directory", project.path().to_string_lossy().into_owned()),
            ("archived", archived.to_string()),
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
            if session_directory(value).is_some_and(|directory| project.matches(directory)) {
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
        self.start_session_with_options(project_path, None, None, None)
    }

    pub fn start_session_with_options(
        &self,
        project_path: impl AsRef<Path>,
        model: Option<&str>,
        effort: Option<&str>,
        agent: Option<&str>,
    ) -> Result<OpenCodeSessionInfo, OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let fallback = if model.is_none() && effort.is_some() {
            let query = [(
                "location[directory]",
                project.path().to_string_lossy().into_owned(),
            )];
            let response = server.request_json(Method::GET, "/api/model/default", &query, None)?;
            Some(response.get("data").and_then(model_id).ok_or_else(|| {
                OpenCodeError::Protocol("no model available for the selected effort".into())
            })?)
        } else {
            None
        };
        // Include the initial variant atomically. Switching it before the first
        // prompt creates an effort-update history item on the provider side.
        let model = model
            .or(fallback.as_deref())
            .map(|model| model_ref(model, effort))
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
        if !session_directory(value).is_some_and(|directory| project.matches(directory)) {
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
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        Ok(OpenCodeSessionInfo {
            project: project.info(),
            session,
        })
    }

    /// Restore metadata and the initial page from one fresh ownership/activity
    /// read. Both belong to the same application request and history generation.
    pub(crate) fn resume_with_history(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(OpenCodeSessionInfo, SessionHistoryPage), OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        let page = history_on(&server, &session, None)?;
        Ok((
            OpenCodeSessionInfo {
                project: project.info(),
                session,
            },
            page,
        ))
    }

    /// Locate an existing session without authorizing any operation in its new
    /// checkout. The application validates the destination before reconnecting.
    pub(crate) fn session_location(
        &self,
        project_path: &str,
        session_id: &str,
    ) -> Result<(caelum_protocol::ProjectInfo, bool), OpenCodeError> {
        if !valid_session_id(session_id) {
            return Err(OpenCodeError::InvalidSessionId);
        }
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let response = server.request_json(
            Method::GET,
            &format!("/api/session/{session_id}"),
            &[],
            None,
        )?;
        let value = &response["data"];
        if value["id"].as_str() != Some(session_id) {
            return Err(OpenCodeError::Protocol(
                "session response has a different session ID".into(),
            ));
        }
        let directory = session_directory(value).ok_or_else(|| {
            OpenCodeError::Protocol("session response has no location directory".into())
        })?;
        if !Path::new(directory).is_absolute() {
            return Err(OpenCodeError::Protocol(
                "session location is not an absolute directory".into(),
            ));
        }
        let destination = self.project(directory)?;
        if destination.path() != project.path() {
            server.forget_session(session_id);
        } else {
            server.remember_session(session_id);
        }
        let archived = value
            .pointer("/time/archived")
            .is_some_and(|v| !v.is_null());
        Ok((destination.info(), archived))
    }

    pub(crate) fn forget_previous_location(&self, project: &str, session_id: &str) {
        if let Some(slot) = self.inner.lock_servers().get(Path::new(project)) {
            slot.forget_session_if_connected(session_id);
        }
    }

    /// Ask OpenCode's title agent to rename a session using its conversation.
    pub fn regenerate_title(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(), OpenCodeError> {
        self.set_title(project_path, session_id, "")
    }

    pub(crate) fn set_title(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        title: &str,
    ) -> Result<(), OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        self.session_metadata(&server, &project, session_id)?;
        server.request_json(
            Method::PATCH,
            &format!("/api/session/{session_id}"),
            &[],
            Some(json!({ "title": title })),
        )?;
        Ok(())
    }

    pub(crate) fn checkpoint_files(
        &self,
        path: &str,
        id: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<String>, OpenCodeError> {
        let project = self.project(path)?;
        let server = self.server(&project)?;
        self.session_metadata(&server, &project, id)?;
        let response = server.request_json(
            Method::GET,
            &format!("/api/session/{id}/diff"),
            &[
                ("from", from.into()),
                ("to", to.into()),
                ("context", "0".into()),
            ],
            None,
        )?;
        response["data"]
            .as_array()
            .ok_or_else(|| OpenCodeError::Protocol("Missing snapshot diff".into()))?
            .iter()
            .map(|file| {
                file["file"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| OpenCodeError::Protocol("Incomplete snapshot diff".into()))
            })
            .collect()
    }

    pub(crate) fn rewind(
        &self,
        path: &str,
        id: &str,
        message: &str,
        tip: &str,
        files: bool,
    ) -> Result<(), OpenCodeError> {
        if !valid_message_id(message) {
            return Err(OpenCodeError::Protocol("Invalid rewind boundary".into()));
        }
        let project = self.project(path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, id)?;
        if session.archived || session.is_working {
            return Err(OpenCodeError::Protocol(
                "Rewind needs an idle, unarchived conversation.".into(),
            ));
        }
        let route = format!("/api/session/{id}");
        // Recheck provider-owned queues and background work in addition to the UI's admission.
        if !self.inbox(path, id)?.is_empty() || !self.background_shells(path, id)?.is_empty() {
            return Err(OpenCodeError::Protocol(
                "Finish queued prompts and background commands before rewinding.".into(),
            ));
        }
        let current = server.request_json(Method::GET, &route, &[], None)?;
        if !current["data"]["revert"].is_null() {
            return Err(OpenCodeError::Protocol(
                "A revert is already staged. Commit or clear it in OpenCode before retrying."
                    .into(),
            ));
        }
        let latest = server.request_json(
            Method::GET,
            &format!("{route}/message"),
            &[("order", "desc".into()), ("limit", "1".into())],
            None,
        )?;
        if latest.pointer("/data/0/id").and_then(Value::as_str) != Some(tip) {
            return Err(OpenCodeError::Protocol(
                "History changed. Reopen /rewind to review the new boundary.".into(),
            ));
        }
        let staged = server.request_json(
            Method::POST,
            &format!("{route}/revert/stage"),
            &[],
            Some(json!({"messageID":message, "files":files})),
        )?;
        if staged.pointer("/data/messageID").and_then(Value::as_str) != Some(message) {
            return Err(OpenCodeError::Protocol(
                "Unexpected staged boundary; inspect the revert in OpenCode before continuing."
                    .into(),
            ));
        }
        let current = server.request_json(Method::GET, &route, &[], None)?;
        if current["data"]["id"].as_str() != Some(id)
            || !session_directory(&current["data"]).is_some_and(|dir| project.matches(dir))
            || current["data"]["revert"] != staged["data"]
        {
            return Err(OpenCodeError::Protocol("The staged revert changed. Inspect it in OpenCode; it was not committed by Caelum.".into()));
        }
        server.request_json(Method::POST, &format!("{route}/revert/commit"), &[], None).map_err(|e| OpenCodeError::Protocol(format!("Could not confirm the staged revert: {e}. Inspect the session in OpenCode and commit or clear the staged boundary.")))?;
        Ok(())
    }

    pub fn session_history(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        cursor: Option<&str>,
    ) -> Result<SessionHistoryPage, OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        history_on(&server, &session, cursor)
    }

    pub fn session_details(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<SessionDetails, OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        self.session_metadata(&server, &project, session_id)?;
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
            branches: Vec::new(),
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
        let project = self.project(project_path)?;
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

    pub fn delete_session(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(), OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        let session = self.session(&server, &project, session_id)?;
        if session.is_working {
            return Err(OpenCodeError::Protocol(
                "stop the running turn before deleting this session".into(),
            ));
        }
        if !session.archived {
            return Err(OpenCodeError::Protocol(
                "only archived OpenCode sessions can be deleted".into(),
            ));
        }
        server.request_json(
            Method::DELETE,
            &format!("/api/session/{session_id}"),
            &[],
            None,
        )?;
        self.inner.questions.clear(
            PROVIDER_NAME,
            &project.path().to_string_lossy(),
            Some(session_id),
        );
        server.forget_session(session_id);
        Ok(())
    }

    pub fn cancel_turn(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(), OpenCodeError> {
        let project = self.project(project_path)?;
        let server = self.server(&project)?;
        self.session_metadata(&server, &project, session_id)?;
        let path = format!("/api/session/{session_id}/interrupt");
        server.request_json(Method::POST, &path, &[], None)?;
        self.inner.questions.clear(
            PROVIDER_NAME,
            &project.path().to_string_lossy(),
            Some(session_id),
        );
        Ok(())
    }

    pub(super) fn session(
        &self,
        server: &OpenCodeServer,
        project: &Project,
        session_id: &str,
    ) -> Result<OpenCodeSession, OpenCodeError> {
        let mut session = self.session_metadata(server, project, session_id)?;
        // Only callers consuming activity need the runtime's fresh active map.
        session.is_working = server.active_sessions()?.contains(&session.id);
        Ok(session)
    }

    pub(super) fn session_metadata(
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
        if !session_directory(value).is_some_and(|directory| project.matches(directory)) {
            return Err(OpenCodeError::SessionNotInProject);
        }
        let session = session_info(value)
            .ok_or_else(|| OpenCodeError::Protocol("session response has no session ID".into()))?;
        if session.id != session_id {
            return Err(OpenCodeError::Protocol(
                "session response has a different session ID".into(),
            ));
        }
        server.remember_session(&session.id);
        Ok(session)
    }
}

fn history_on(
    server: &OpenCodeServer,
    session: &OpenCodeSession,
    cursor: Option<&str>,
) -> Result<SessionHistoryPage, OpenCodeError> {
    let path = format!("/api/session/{}/message", session.id);
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
        model: session.model.clone(),
        effort: session.effort.clone(),
        agent: session.agent.clone(),
        data: messages.iter().rev().cloned().collect(),
        next_cursor: serde_json::from_value(response["cursor"]["next"].clone()).map_err(|_| {
            OpenCodeError::Protocol("Session history returned an invalid cursor".into())
        })?,
    })
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
        archived: value
            .pointer("/time/archived")
            .is_some_and(|archived| !archived.is_null()),
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

fn valid_message_id(id: &str) -> bool {
    id.starts_with("msg_")
        && id.len() > 4
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

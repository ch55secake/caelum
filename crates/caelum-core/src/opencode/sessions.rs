use super::{
    directory_matches, model_id, model_ref, valid_session_id, OpenCodeError, OpenCodeProvider,
    OpenCodeServer, PROVIDER_NAME, SESSION_LIMIT,
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
    pub fn list_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        Self::list_sessions_on(&project, &server, false)
    }

    /// Return archived OpenCode sessions for this project.
    pub fn list_archived_sessions(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<OpenCodeSessionList, OpenCodeError> {
        let project = Project::open(project_path)?;
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

    pub fn delete_session(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<(), OpenCodeError> {
        let project = Project::open(project_path)?;
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

    pub(super) fn session(
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

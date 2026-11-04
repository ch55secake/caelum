use super::{OpenCodeError, OpenCodeProvider};
use crate::projects::Project;
use caelum_protocol::OpenCodeBackgroundShell;
use reqwest::Method;
use serde_json::json;
use std::path::Path;

impl OpenCodeProvider {
    pub fn background_shells(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<Vec<OpenCodeBackgroundShell>, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let response = server.request_json(
            Method::GET,
            "/api/experimental/job",
            &[
                ("sessionID", session_id.to_owned()),
                ("kind", "shell".to_owned()),
                ("status", "running".to_owned()),
                ("descendants", "false".to_owned()),
            ],
            None,
        )?;
        let jobs = response["data"].as_array().ok_or_else(|| {
            OpenCodeError::Protocol("background job response has no data array".into())
        })?;
        // Enforce ownership even if the experimental endpoint ignores its filters.
        jobs.iter()
            .filter(|job| {
                job["sessionID"].as_str() == Some(session_id)
                    && job["kind"] == "shell"
                    && job["status"] == "running"
            })
            .map(|job| {
                serde_json::from_value(json!({
                    "id": job["shellID"],
                    "command": job["command"],
                    "startedAt": job["time"]["started"],
                }))
                .map_err(|error| {
                    OpenCodeError::Protocol(format!("invalid background shell: {error}"))
                })
            })
            .collect()
    }
}

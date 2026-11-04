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
            "/api/shell",
            &[(
                "location[directory]",
                project.path().to_string_lossy().into_owned(),
            )],
            None,
        )?;
        let shells = response["data"]
            .as_array()
            .ok_or_else(|| OpenCodeError::Protocol("shell response has no data array".into()))?;
        // The location list includes other sessions and foreground commands.
        // Only owned commands count; the UI shows waiting once the model is idle.
        shells
            .iter()
            .filter(|shell| {
                shell["metadata"]["sessionID"].as_str() == Some(session_id)
                    && shell["status"] == "running"
            })
            .map(|shell| {
                serde_json::from_value(json!({
                    "id": shell["id"],
                    "command": shell["command"],
                    "startedAt": shell["time"]["started"],
                }))
                .map_err(|error| {
                    OpenCodeError::Protocol(format!("invalid background shell: {error}"))
                })
            })
            .collect()
    }
}

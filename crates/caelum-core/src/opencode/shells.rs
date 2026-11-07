use super::{OpenCodeError, OpenCodeProvider};
use crate::projects::Project;
use caelum_protocol::OpenCodeBackgroundShell;
use reqwest::Method;
use serde_json::json;
use std::path::Path;

impl OpenCodeProvider {
    /// Read or stop only a shell belonging to the captured session and checkout.
    pub fn shell_action(
        &self,
        project_path: &str,
        session_id: &str,
        id: &str,
        stop: bool,
    ) -> Result<String, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(OpenCodeError::Protocol("Invalid shell ID".into()));
        }
        let path = format!("/api/shell/{id}");
        let query = [(
            "location[directory]",
            project.path().to_string_lossy().into_owned(),
        )];
        let info = server.request_json(Method::GET, &path, &query, None)?;
        if info["data"]["metadata"]["sessionID"].as_str() != Some(session_id) {
            return Err(OpenCodeError::Protocol(
                "Shell belongs to another session".into(),
            ));
        }
        let read = || {
            let mut output = server.request_json(
                Method::GET,
                &format!("{path}/output"),
                &[query[0].clone(), ("limit", "12000".into())],
                None,
            )?;
            if let Some(size) = output["data"]["size"].as_u64().filter(|n| *n > 12000) {
                output = server.request_json(
                    Method::GET,
                    &format!("{path}/output"),
                    &[
                        query[0].clone(),
                        ("cursor", (size - 12000).to_string()),
                        ("limit", "12000".into()),
                    ],
                    None,
                )?;
            }
            let output = output["data"]["output"]
                .as_str()
                .ok_or_else(|| OpenCodeError::Protocol("shell output has no text".into()))?;
            let start = output.ceil_char_boundary(output.len().saturating_sub(12000));
            Ok(output[start..].to_owned())
        };
        if stop {
            // DELETE also removes retained output. Keep the final visible tail,
            // but an output failure must never prevent stopping a command.
            let output = read().unwrap_or_default();
            server.request_json(Method::DELETE, &path, &query, None)?;
            Ok(output)
        } else {
            read()
        }
    }

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

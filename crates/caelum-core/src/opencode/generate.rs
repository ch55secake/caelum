use super::{OpenCodeError, OpenCodeProvider};
use crate::projects::Project;
use reqwest::Method;
use serde_json::json;
use std::path::Path;

impl OpenCodeProvider {
    /// Generate transient text without enqueueing a turn or changing session settings.
    pub fn generate_from_context(
        &self,
        project_path: impl AsRef<Path>,
        session_id: &str,
        prompt: &str,
    ) -> Result<String, OpenCodeError> {
        if prompt.trim().is_empty() {
            return Err(OpenCodeError::Protocol(
                "side question must not be empty".into(),
            ));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        self.session(&server, &project, session_id)?;
        let response = server.request_json(
            Method::POST,
            &format!("/api/session/{session_id}/generate"),
            &[],
            Some(json!({ "prompt": prompt })),
        )?;
        response["data"]["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| OpenCodeError::Protocol("side question response has no text".into()))
    }
}

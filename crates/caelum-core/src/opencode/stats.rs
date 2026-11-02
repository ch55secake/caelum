use super::{OpenCodeError, OpenCodeProvider};
use crate::projects::Project;
use caelum_protocol::{OpenCodeStats, OpenCodeStatsFilter, OpenCodeStatsProject};
use reqwest::Method;
use std::path::Path;

impl OpenCodeProvider {
    pub fn stats_projects(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<Vec<OpenCodeStatsProject>, OpenCodeError> {
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let response = server.request_json(
            Method::GET,
            "/api/project",
            &[("archived", "true".to_owned())],
            None,
        )?;
        serde_json::from_value(response)
            .map_err(|error| OpenCodeError::Protocol(format!("invalid project list: {error}")))
    }

    pub fn session_stats(
        &self,
        project_path: impl AsRef<Path>,
        filter: &OpenCodeStatsFilter,
    ) -> Result<OpenCodeStats, OpenCodeError> {
        if matches!((filter.from, filter.to), (Some(from), Some(to)) if from >= to) {
            return Err(OpenCodeError::Protocol(
                "stats end must be after start".into(),
            ));
        }
        let project = Project::open(project_path)?;
        let server = self.server(&project)?;
        let mut query = vec![("tools", "none".to_owned())];
        if let Some(from) = filter.from {
            query.push(("from", from.to_string()));
        }
        if let Some(to) = filter.to {
            query.push(("to", to.to_string()));
        }
        if let Some(project) = &filter.project {
            query.push(("project", project.clone()));
        }
        if let Some(timezone) = &filter.timezone {
            query.push(("timezone", timezone.clone()));
        }
        let response =
            server.request_json(Method::GET, "/api/experimental/session/stats", &query, None)?;
        serde_json::from_value(response.get("data").cloned().unwrap_or_default())
            .map_err(|error| OpenCodeError::Protocol(format!("invalid statistics: {error}")))
    }
}

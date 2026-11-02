use caelum_core::database::WorkspaceDatabase;
use caelum_protocol::ProjectInfo;
use std::path::PathBuf;

pub(crate) struct PersistenceState {
    database: Option<WorkspaceDatabase>,
    unavailable_reason: Option<String>,
}

pub(crate) struct StartupProject(pub(crate) Option<ProjectInfo>);

impl PersistenceState {
    pub(crate) fn open(app_data_dir: Result<PathBuf, tauri::Error>) -> Self {
        match app_data_dir {
            Ok(path) => match WorkspaceDatabase::open(path.join("workspace.sqlite3")) {
                Ok(database) => Self {
                    database: Some(database),
                    unavailable_reason: None,
                },
                Err(error) => Self::unavailable(error.to_string()),
            },
            Err(error) => Self::unavailable(error.to_string()),
        }
    }

    fn unavailable(reason: String) -> Self {
        Self {
            database: None,
            unavailable_reason: Some(reason),
        }
    }

    pub(crate) fn database(&self) -> Result<WorkspaceDatabase, String> {
        self.database.clone().ok_or_else(|| {
            format!(
                "Local workspace persistence is unavailable: {}",
                self.unavailable_reason
                    .as_deref()
                    .unwrap_or("unknown error")
            )
        })
    }
}

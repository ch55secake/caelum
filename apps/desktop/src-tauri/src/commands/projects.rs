use crate::runtime::persistence::StartupProject;
use caelum_core::projects::Project;
use caelum_protocol::{AppInfo, ProjectInfo};
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub(crate) fn app_info() -> AppInfo {
    AppInfo {
        name: "Caelum".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
pub(crate) fn startup_project(project: State<'_, StartupProject>) -> Option<ProjectInfo> {
    project.0.clone()
}

#[tauri::command]
pub(crate) fn open_project(path: String) -> Result<ProjectInfo, String> {
    Project::open(PathBuf::from(path))
        .map(|project| project.info())
        .map_err(|error| error.to_string())
}

#![forbid(unsafe_code)]

use std::path::PathBuf;

use caelum_core::projects::Project;
use caelum_protocol::{AppInfo, ProjectInfo};

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Caelum".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
fn open_project(path: String) -> Result<ProjectInfo, String> {
    Project::open(PathBuf::from(path))
        .map(|project| project.info())
        .map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_info, open_project])
        .run(tauri::generate_context!())
        .expect("error while running Caelum desktop application");
}

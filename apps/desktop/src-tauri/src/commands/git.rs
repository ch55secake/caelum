use caelum_core::git::GitClient;
use caelum_core::session_branches::SessionBranches;
use caelum_protocol::{GitFileDiff, GitFileVersions, GitRepositoryStatus};
use tauri::{Emitter, State};

#[tauri::command]
pub(crate) async fn git_repository_status(
    app: tauri::AppHandle,
    branches: State<'_, SessionBranches>,
    project_path: String,
) -> Result<GitRepositoryStatus, String> {
    let branches = branches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = GitClient::new()
            .repository_status(project_path)
            .map_err(|error| error.to_string())?;
        for update in branches.refresh_checkout(&status) {
            let _ = app.emit("session-branch-updated", update);
        }
        Ok(status)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn git_file_diff(
    project_path: String,
    file_path: String,
) -> Result<GitFileDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        GitClient::new()
            .file_diff(project_path, &file_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn git_file_versions(
    project_path: String,
    file_path: String,
) -> Result<GitFileVersions, String> {
    tauri::async_runtime::spawn_blocking(move || {
        GitClient::new()
            .file_versions(project_path, &file_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

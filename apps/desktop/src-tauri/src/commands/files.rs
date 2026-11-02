use caelum_core::files;
use caelum_core::opencode::OpenCodeProvider;
use caelum_protocol::{
    ProjectDirectoryListing, ProjectFileContent, ProjectSearchMode, ProjectSearchResults,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_project_directory(
    project_path: String,
    path: String,
    offset: usize,
    limit: usize,
) -> Result<ProjectDirectoryListing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        files::list_directory(project_path, &path, offset, limit).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn search_project_files(
    project_path: String,
    query: String,
    mode: ProjectSearchMode,
    include_ignored: bool,
) -> Result<ProjectSearchResults, String> {
    tauri::async_runtime::spawn_blocking(move || {
        files::search_project(project_path, &query, mode, include_ignored)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn read_project_image(
    project_path: String,
    path: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || files::read_image(project_path, &path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_read_image(
    provider: State<'_, OpenCodeProvider>,
    project_path: String,
    hash: String,
) -> Result<String, String> {
    let provider = provider.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        provider
            .read_image(project_path, &hash)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn read_project_file(
    project_path: String,
    path: String,
) -> Result<ProjectFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || {
        files::read_text_file(project_path, &path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

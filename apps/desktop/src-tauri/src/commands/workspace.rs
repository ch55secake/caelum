use crate::state::PersistenceState;
use caelum_protocol::{ProjectInfo, WorkspaceSnapshot};
use tauri::State;

#[tauri::command]
pub(crate) async fn restore_workspace(
    persistence: State<'_, PersistenceState>,
) -> Result<WorkspaceSnapshot, String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .restore()
            .map_err(|error| format!("Could not restore the local workspace: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn remember_project(
    persistence: State<'_, PersistenceState>,
    project: ProjectInfo,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .remember_project(&project)
            .map_err(|error| format!("Could not save the project: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn close_project_record(
    persistence: State<'_, PersistenceState>,
    project_path: String,
    active_project_path: Option<String>,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .close_project(&project_path, active_project_path.as_deref())
            .map_err(|error| format!("Could not update the open projects: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn set_active_project(
    persistence: State<'_, PersistenceState>,
    project_path: Option<String>,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .set_active_project(project_path.as_deref())
            .map_err(|error| format!("Could not save the active project: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn set_project_provider(
    persistence: State<'_, PersistenceState>,
    project_path: String,
    provider: String,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .set_provider(&project_path, &provider)
            .map_err(|error| format!("Could not save the selected provider: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn select_project_session(
    persistence: State<'_, PersistenceState>,
    project_path: String,
    provider: String,
    session_id: Option<String>,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .select_session(&project_path, &provider, session_id.as_deref())
            .map_err(|error| format!("Could not save the selected session: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn sync_provider_sessions(
    persistence: State<'_, PersistenceState>,
    project_path: String,
    provider: String,
    session_ids: Vec<String>,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .sync_sessions(&project_path, &provider, &session_ids)
            .map_err(|error| format!("Could not save session metadata: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn forget_project_session(
    persistence: State<'_, PersistenceState>,
    project_path: String,
    provider: String,
    session_id: String,
) -> Result<(), String> {
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        database
            .remove_session(&project_path, &provider, &session_id)
            .map_err(|error| format!("Could not remove the saved session: {error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

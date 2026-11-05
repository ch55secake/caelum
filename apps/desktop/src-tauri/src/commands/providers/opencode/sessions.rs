use caelum_core::opencode::{OpenCodeError, OpenCodeProvider};
use caelum_protocol::{
    OpenCodeBackgroundShell, OpenCodeSessionInfo, OpenCodeSessionList, SessionHistoryPage,
    SessionListError,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn opencode_list_sessions(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<OpenCodeSessionList, SessionListError> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .list_sessions(project_path)
            .map_err(|error| match error {
                OpenCodeError::NotInstalled => SessionListError::NotInstalled(error.to_string()),
                error => SessionListError::Failed(error.to_string()),
            })
    })
    .await
    .map_err(|error| SessionListError::Failed(error.to_string()))?
}

#[tauri::command]
pub(crate) async fn opencode_start_session(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    model: Option<String>,
    agent: Option<String>,
) -> Result<OpenCodeSessionInfo, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .start_session_with_options(project_path, model.as_deref(), agent.as_deref())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_resume_session(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<OpenCodeSessionInfo, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .resume_session(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_session_history(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    cursor: Option<String>,
) -> Result<SessionHistoryPage, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .session_history(project_path, &session_id, cursor.as_deref())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_background_shells(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<Vec<OpenCodeBackgroundShell>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .background_shells(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

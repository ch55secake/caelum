use caelum_core::codex::{CodexError, CodexProvider};
use caelum_protocol::{CodexSessionInfo, CodexSessionList, SessionHistoryPage, SessionListError};
use tauri::State;

#[tauri::command]
pub(crate) async fn codex_list_sessions(
    codex: State<'_, CodexProvider>,
    project_path: String,
) -> Result<CodexSessionList, SessionListError> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .list_sessions(project_path)
            .map_err(|error| match error {
                CodexError::NotInstalled => SessionListError::NotInstalled(error.to_string()),
                error => SessionListError::Failed(error.to_string()),
            })
    })
    .await
    .map_err(|error| SessionListError::Failed(error.to_string()))?
}

#[tauri::command]
pub(crate) async fn codex_start_session(
    codex: State<'_, CodexProvider>,
    project_path: String,
    model: Option<String>,
) -> Result<CodexSessionInfo, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .start_session_with_model(project_path, model.as_deref())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn codex_resume_session(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
) -> Result<CodexSessionInfo, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .resume_session(project_path, &thread_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn codex_session_history(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
    cursor: Option<String>,
) -> Result<SessionHistoryPage, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .session_history(project_path, &thread_id, cursor.as_deref())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn codex_send_prompt(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
    prompt: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .send_prompt_with_options(
                project_path,
                &thread_id,
                &prompt,
                model.as_deref(),
                effort.as_deref(),
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn codex_cancel_turn(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .cancel_turn(project_path, &thread_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

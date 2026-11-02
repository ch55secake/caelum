use caelum_core::opencode::{OpenCodeError, OpenCodeProvider};
use caelum_protocol::{
    OpenCodeSessionInfo, OpenCodeSessionList, SessionHistoryPage, SessionListError,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn opencode_generate(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    prompt: String,
) -> Result<String, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .generate_from_context(project_path, &session_id, &prompt)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_stats_projects(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<Vec<caelum_protocol::OpenCodeStatsProject>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .stats_projects(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_stats(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    filter: caelum_protocol::OpenCodeStatsFilter,
) -> Result<caelum_protocol::OpenCodeStats, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .session_stats(project_path, &filter)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

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
pub(crate) async fn opencode_select_agent(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    agent: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .select_agent(project_path, &session_id, &agent)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_enqueue_prompt(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    message_id: String,
    prompt: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .enqueue_prompt(project_path, &session_id, &message_id, &prompt)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_inbox(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .inbox(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_cancel_queued_prompt(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    message_id: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .cancel_queued_prompt(project_path, &session_id, &message_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_send_prompt(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    prompt: String,
    message_id: Option<String>,
    model: Option<String>,
    effort: Option<String>,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .send_prompt_with_options(
                project_path,
                &session_id,
                &prompt,
                message_id.as_deref(),
                model.as_deref(),
                effort.as_deref(),
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_cancel_turn(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .cancel_turn(project_path, &session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_commands(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<Vec<caelum_protocol::OpenCodeCommandInfo>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .commands(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_usage_limits(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<serde_json::Value, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .usage_limits(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn opencode_run_command(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    session_id: String,
    command: caelum_protocol::OpenCodeCommandInput,
    model: Option<String>,
    effort: Option<String>,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .run_command(
                project_path,
                &session_id,
                &command,
                model.as_deref(),
                effort.as_deref(),
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

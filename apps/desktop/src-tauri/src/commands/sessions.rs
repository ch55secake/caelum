use caelum_core::codex::CodexProvider;
use caelum_core::opencode::OpenCodeProvider;
use caelum_core::session_branches::SessionBranches;
use caelum_protocol::{ProviderChoices, SessionDetails};
use tauri::State;

#[tauri::command]
pub(crate) async fn session_details(
    codex: State<'_, CodexProvider>,
    opencode: State<'_, OpenCodeProvider>,
    branches: State<'_, SessionBranches>,
    project_path: String,
    provider: String,
    session_id: String,
) -> Result<SessionDetails, String> {
    let codex = codex.inner().clone();
    let opencode = opencode.inner().clone();
    let branches = branches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let details = match provider.as_str() {
            "codex" => codex
                .session_details(&project_path, &session_id)
                .map_err(|error| error.to_string()),
            "opencode" => opencode
                .session_details(&project_path, &session_id)
                .map_err(|error| error.to_string()),
            _ => Err("Unknown provider".into()),
        }?;
        Ok(branches.details(&project_path, &provider, &session_id, details))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn set_session_archived(
    codex: State<'_, CodexProvider>,
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    provider: String,
    session_id: String,
    archived: bool,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
        "codex" => codex
            .set_session_archived(project_path, &session_id, archived)
            .map_err(|error| error.to_string()),
        "opencode" => opencode
            .set_session_archived(project_path, &session_id, archived)
            .map_err(|error| error.to_string()),
        _ => Err("Unknown provider".into()),
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn provider_choices(
    codex: State<'_, CodexProvider>,
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
    provider: String,
) -> Result<ProviderChoices, String> {
    let codex = codex.inner().clone();
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
        "codex" => codex
            .choices(project_path)
            .map_err(|error| error.to_string()),
        "opencode" => opencode
            .choices(project_path)
            .map_err(|error| error.to_string()),
        _ => Err("Unknown provider".into()),
    })
    .await
    .map_err(|error| error.to_string())?
}

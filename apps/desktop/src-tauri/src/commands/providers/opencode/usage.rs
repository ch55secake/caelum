use caelum_core::opencode::OpenCodeProvider;
use caelum_protocol::{OpenCodeStats, OpenCodeStatsFilter, OpenCodeStatsProject};
use tauri::State;

#[tauri::command]
pub(crate) async fn opencode_stats_projects(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<Vec<OpenCodeStatsProject>, String> {
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
    filter: OpenCodeStatsFilter,
) -> Result<OpenCodeStats, String> {
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

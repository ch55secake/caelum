use crate::state::PersistenceState;
use caelum_core::approvals::ApprovalBroker;
use caelum_core::codex::CodexProvider;
use caelum_core::opencode::OpenCodeProvider;
use caelum_core::projects::Project;
use caelum_core::questions::QuestionBroker;
use caelum_core::worktrees::WorktreeClient;
use caelum_protocol::{GitWorktree, GitWorktrees, ProjectInfo};
use std::path::PathBuf;
use tauri::{Manager, State};

#[tauri::command]
pub(crate) async fn git_worktrees(
    client: State<'_, WorktreeClient>,
    project_path: String,
    base_directory: String,
) -> Result<Option<GitWorktrees>, String> {
    let client = client.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        client
            .list(project_path, &base_directory)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn worktree_destination(
    client: State<'_, WorktreeClient>,
    project_path: String,
    branch: String,
    base_directory: String,
) -> Result<String, String> {
    let client = client.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        client
            .destination(project_path, &branch, &base_directory)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn create_worktree(
    client: State<'_, WorktreeClient>,
    persistence: State<'_, PersistenceState>,
    project_path: String,
    branch: String,
    base: String,
    base_directory: String,
) -> Result<GitWorktree, String> {
    let client = client.inner().clone();
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        let listing = client
            .list(&project_path, &base_directory)
            .map_err(|error| error.to_string())?
            .ok_or("Open a Git project first.")?;
        let entry = client
            .create(&project_path, &branch, &base, &base_directory)
            .map_err(|error| error.to_string())?;
        database
            .remember_worktree(&listing.project, &entry.project)
            .map_err(|error| {
                format!("Worktree created, but its workspace could not be saved: {error}")
            })?;
        Ok(entry)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn open_worktree(
    client: State<'_, WorktreeClient>,
    persistence: State<'_, PersistenceState>,
    project_path: String,
    worktree_path: String,
) -> Result<ProjectInfo, String> {
    let client = client.inner().clone();
    let database = persistence.database()?;
    tauri::async_runtime::spawn_blocking(move || {
        let listing = client
            .list(project_path, "")
            .map_err(|error| error.to_string())?
            .ok_or("Open a Git project first.")?;
        let entry = listing
            .worktrees
            .iter()
            .find(|entry| entry.path == worktree_path && entry.available)
            .ok_or("This worktree is unavailable. Refresh the worktree list.")?;
        let mut project = Project::open(&entry.project.path)
            .map_err(|error| error.to_string())?
            .info();
        project.name.clone_from(&listing.project.name);
        database
            .remember_worktree(&listing.project, &project)
            .map_err(|error| error.to_string())?;
        Ok(project)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn remove_worktree(
    app: tauri::AppHandle,
    project_path: String,
    worktree_path: String,
) -> Result<(), String> {
    let client = app.state::<WorktreeClient>().inner().clone();
    let database = app.state::<PersistenceState>().database()?;
    let codex = app.state::<CodexProvider>().inner().clone();
    let opencode = app.state::<OpenCodeProvider>().inner().clone();
    let approvals = app.state::<ApprovalBroker>().inner().clone();
    let questions = app.state::<QuestionBroker>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let checkout = PathBuf::from(&worktree_path);
        if approvals
            .pending()
            .iter()
            .any(|request| PathBuf::from(&request.project_path).starts_with(&checkout))
            || questions
                .pending()
                .iter()
                .any(|request| PathBuf::from(&request.project_path).starts_with(&checkout))
        {
            return Err("Resolve pending agent requests before removing this worktree.".into());
        }
        codex
            .ensure_checkout_idle(&checkout)
            .map_err(|error| error.to_string())?;
        opencode
            .ensure_checkout_idle(&checkout)
            .map_err(|error| error.to_string())?;
        let removed = client
            .remove(project_path, &worktree_path)
            .map_err(|error| error.to_string())?;
        codex.release_checkout(&checkout);
        opencode.release_checkout(&checkout);
        database
            .remove_worktree(&removed.path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

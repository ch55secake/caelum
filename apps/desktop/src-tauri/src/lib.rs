#![forbid(unsafe_code)]

use std::{path::PathBuf, sync::Arc};

use caelum_core::{
    approvals::ApprovalBroker,
    codex::CodexProvider,
    database::WorkspaceDatabase,
    files,
    git::GitClient,
    github,
    opencode::OpenCodeProvider,
    processes::ProcessRegistry,
    projects::Project,
    questions::{Answer, QuestionBroker},
    session_branches::SessionBranches,
    worktrees::WorktreeClient,
};
use caelum_protocol::{
    AppInfo, ApprovalDecision, ApprovalPrompt, CodexSessionInfo, CodexSessionList, GitFileDiff,
    GitFileVersions, GitHubFeedbackKind, GitHubMergeMethod, GitHubPullRequest,
    GitHubPullRequestDetails, GitHubPullRequestFeedback, GitHubPullRequestFiles,
    GitRepositoryStatus, GitWorktree, GitWorktrees, OpenCodeSessionInfo, OpenCodeSessionList,
    ProjectDirectoryListing, ProjectFileContent, ProjectInfo, ProjectSearchMode,
    ProjectSearchResults, ProviderChoices, QuestionPrompt, SessionDetails, SessionHistoryPage,
    WorkspaceSnapshot,
};
use tauri::{Emitter, Manager, State};

struct PersistenceState {
    database: Option<WorkspaceDatabase>,
    unavailable_reason: Option<String>,
}

struct StartupProject(Option<ProjectInfo>);

impl PersistenceState {
    fn open(app_data_dir: Result<PathBuf, tauri::Error>) -> Self {
        match app_data_dir {
            Ok(path) => match WorkspaceDatabase::open(path.join("workspace.sqlite3")) {
                Ok(database) => Self {
                    database: Some(database),
                    unavailable_reason: None,
                },
                Err(error) => Self::unavailable(error.to_string()),
            },
            Err(error) => Self::unavailable(error.to_string()),
        }
    }

    fn unavailable(reason: String) -> Self {
        Self {
            database: None,
            unavailable_reason: Some(reason),
        }
    }

    fn database(&self) -> Result<WorkspaceDatabase, String> {
        self.database.clone().ok_or_else(|| {
            format!(
                "Local workspace persistence is unavailable: {}",
                self.unavailable_reason
                    .as_deref()
                    .unwrap_or("unknown error")
            )
        })
    }
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Caelum".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

#[tauri::command]
fn startup_project(project: State<'_, StartupProject>) -> Option<ProjectInfo> {
    project.0.clone()
}

#[tauri::command]
fn open_project(path: String) -> Result<ProjectInfo, String> {
    Project::open(PathBuf::from(path))
        .map(|project| project.info())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn git_worktrees(
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
async fn worktree_destination(
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
async fn create_worktree(
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
async fn open_worktree(
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
async fn remove_worktree(
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

#[tauri::command]
async fn git_repository_status(
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
async fn github_open_pull_request(
    project_path: String,
    branch: String,
) -> Result<Option<GitHubPullRequest>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::open_pull_request(project_path, &branch).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn github_pull_request_details(
    project_path: String,
    url: String,
) -> Result<GitHubPullRequestDetails, String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::pull_request_details(project_path, &url).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn github_pull_request_files(
    project_path: String,
    url: String,
    head_oid: String,
    page: u32,
) -> Result<GitHubPullRequestFiles, String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::pull_request_files(project_path, &url, &head_oid, page)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn github_pull_request_feedback(
    project_path: String,
    url: String,
    head_oid: String,
    kind: GitHubFeedbackKind,
    cursor: Option<String>,
    thread_id: Option<String>,
) -> Result<GitHubPullRequestFeedback, String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::pull_request_feedback(
            project_path,
            &url,
            &head_oid,
            kind,
            cursor.as_deref(),
            thread_id.as_deref(),
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn github_merge_pull_request(
    project_path: String,
    url: String,
    method: GitHubMergeMethod,
    head_oid: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::merge_pull_request(project_path, &url, method, &head_oid)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn github_update_pull_request_branch(
    project_path: String,
    url: String,
    head_oid: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        github::update_pull_request_branch(project_path, &url, &head_oid)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn git_file_diff(project_path: String, file_path: String) -> Result<GitFileDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        GitClient::new()
            .file_diff(project_path, &file_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn git_file_versions(
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

#[tauri::command]
async fn list_project_directory(
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
async fn search_project_files(
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
async fn read_project_image(project_path: String, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || files::read_image(project_path, &path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_read_image(
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
async fn read_project_file(
    project_path: String,
    path: String,
) -> Result<ProjectFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || {
        files::read_text_file(project_path, &path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn restore_workspace(
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
async fn remember_project(
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
async fn close_project_record(
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
async fn set_active_project(
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
async fn set_project_provider(
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
async fn select_project_session(
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
async fn sync_provider_sessions(
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
async fn forget_project_session(
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

#[tauri::command]
async fn session_details(
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
async fn set_session_archived(
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
fn pending_approvals(approvals: State<'_, ApprovalBroker>) -> Vec<ApprovalPrompt> {
    approvals.pending()
}

#[tauri::command]
fn pending_questions(questions: State<'_, QuestionBroker>) -> Vec<QuestionPrompt> {
    questions.pending()
}

#[tauri::command]
async fn refresh_questions(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .refresh_questions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn resolve_question(
    questions: State<'_, QuestionBroker>,
    provider: String,
    project_path: String,
    session_id: String,
    request_id: String,
    answer: Option<Answer>,
) -> Result<(), String> {
    let questions = questions.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        questions.resolve(&provider, &project_path, &session_id, &request_id, answer)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn resolve_approval(
    approvals: State<'_, ApprovalBroker>,
    provider: String,
    project_path: String,
    request_id: String,
    decision: ApprovalDecision,
) -> Result<(), String> {
    approvals
        .resolve(&provider, &project_path, &request_id, decision)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn codex_list_sessions(
    codex: State<'_, CodexProvider>,
    project_path: String,
) -> Result<CodexSessionList, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .list_sessions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn provider_choices(
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

#[tauri::command]
async fn codex_start_session(
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
async fn codex_resume_session(
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
async fn codex_session_history(
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
async fn codex_send_prompt(
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
async fn codex_cancel_turn(
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

#[tauri::command]
async fn opencode_list_sessions(
    opencode: State<'_, OpenCodeProvider>,
    project_path: String,
) -> Result<OpenCodeSessionList, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .list_sessions(project_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn opencode_start_session(
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
async fn opencode_resume_session(
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
async fn opencode_session_history(
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
async fn opencode_select_agent(
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
async fn opencode_send_prompt(
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
async fn opencode_cancel_turn(
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
async fn opencode_commands(
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
async fn opencode_usage_limits(
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
async fn opencode_run_command(
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

pub fn run() {
    let app = tauri::Builder::default()
        .manage(ApprovalBroker::default())
        .manage(QuestionBroker::default())
        .manage(Arc::new(ProcessRegistry::default()))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir();
            app.manage(WorktreeClient::new(app.path().home_dir()?));
            app.manage(PersistenceState::open(app_data_dir));
            app.manage(SessionBranches::new(
                app.state::<PersistenceState>().database().ok(),
            ));
            let startup_project = std::env::args_os()
                .skip(1)
                .map(PathBuf::from)
                .find(|path| path.is_dir())
                .and_then(|path| Project::open(path).ok().map(|project| project.info()));
            app.manage(StartupProject(startup_project));

            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(CodexProvider::new(
                processes,
                approvals,
                app.state::<QuestionBroker>().inner().clone(),
            ));

            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(OpenCodeProvider::new(
                processes,
                approvals,
                app.state::<QuestionBroker>().inner().clone(),
            ));

            let updates = app.state::<QuestionBroker>().subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(prompts) = updates.recv() {
                    if handle.emit("question-requests", prompts).is_err() {
                        break;
                    }
                }
            });

            let updates = app.state::<ApprovalBroker>().subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(prompts) = updates.recv() {
                    if handle.emit("approval-requests", prompts).is_err() {
                        break;
                    }
                }
            });

            let codex_updates = app.state::<CodexProvider>().subscribe();
            let branches = app.state::<SessionBranches>().inner().clone();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = codex_updates.recv() {
                    if let Some(update) = branches.codex_event(&event) {
                        let _ = handle.emit("session-branch-updated", update);
                    }
                    if handle.emit("codex-event", event).is_err() {
                        break;
                    }
                }
            });

            let opencode_updates = app.state::<OpenCodeProvider>().subscribe();
            let branches = app.state::<SessionBranches>().inner().clone();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = opencode_updates.recv() {
                    if let Some(update) = branches.opencode_event(&event) {
                        let _ = handle.emit("session-branch-updated", update);
                    }
                    if handle.emit("opencode-event", event).is_err() {
                        break;
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            startup_project,
            open_project,
            git_worktrees,
            worktree_destination,
            create_worktree,
            open_worktree,
            remove_worktree,
            git_repository_status,
            github_open_pull_request,
            github_pull_request_details,
            github_pull_request_files,
            github_pull_request_feedback,
            github_merge_pull_request,
            github_update_pull_request_branch,
            git_file_diff,
            git_file_versions,
            list_project_directory,
            read_project_file,
            read_project_image,
            opencode_read_image,
            search_project_files,
            restore_workspace,
            remember_project,
            close_project_record,
            set_active_project,
            set_project_provider,
            select_project_session,
            sync_provider_sessions,
            forget_project_session,
            session_details,
            set_session_archived,
            pending_approvals,
            resolve_approval,
            pending_questions,
            refresh_questions,
            resolve_question,
            codex_list_sessions,
            codex_start_session,
            provider_choices,
            codex_resume_session,
            codex_session_history,
            codex_send_prompt,
            codex_cancel_turn,
            opencode_list_sessions,
            opencode_start_session,
            opencode_resume_session,
            opencode_session_history,
            opencode_send_prompt,
            opencode_select_agent,
            opencode_commands,
            opencode_run_command,
            opencode_usage_limits,
            opencode_cancel_turn
        ])
        .build(tauri::generate_context!())
        .expect("error while running Caelum desktop application");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            handle.state::<ApprovalBroker>().shutdown();
            handle.state::<CodexProvider>().shutdown();
            handle.state::<OpenCodeProvider>().shutdown();
            handle.state::<Arc<ProcessRegistry>>().shutdown();
        }
    });
}

#![forbid(unsafe_code)]

use std::{path::PathBuf, sync::Arc};

use caelum_core::{
    approvals::ApprovalBroker, codex::CodexProvider, database::WorkspaceDatabase, files,
    git::GitClient, opencode::OpenCodeProvider, processes::ProcessRegistry, projects::Project,
};
use caelum_protocol::{
    AppInfo, ApprovalDecision, ApprovalPrompt, CodexSessionInfo, CodexSessionList, GitFileDiff,
    GitFileVersions, GitRepositoryStatus, OpenCodeConversationEntry, OpenCodeSessionInfo,
    OpenCodeSessionList, ProjectDirectoryListing, ProjectFileContent, ProjectInfo,
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
async fn git_repository_status(project_path: String) -> Result<GitRepositoryStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        GitClient::new()
            .repository_status(project_path)
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
fn pending_approvals(approvals: State<'_, ApprovalBroker>) -> Vec<ApprovalPrompt> {
    approvals.pending()
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
async fn codex_start_session(
    codex: State<'_, CodexProvider>,
    project_path: String,
) -> Result<CodexSessionInfo, String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .start_session(project_path)
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
async fn codex_send_prompt(
    codex: State<'_, CodexProvider>,
    project_path: String,
    thread_id: String,
    prompt: String,
) -> Result<(), String> {
    let codex = codex.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        codex
            .send_prompt(project_path, &thread_id, &prompt)
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
) -> Result<OpenCodeSessionInfo, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .start_session(project_path)
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
) -> Result<Vec<OpenCodeConversationEntry>, String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .session_history(project_path, &session_id)
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
) -> Result<(), String> {
    let opencode = opencode.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        opencode
            .send_prompt(project_path, &session_id, &prompt)
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

pub fn run() {
    let app = tauri::Builder::default()
        .manage(ApprovalBroker::default())
        .manage(Arc::new(ProcessRegistry::default()))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir();
            app.manage(PersistenceState::open(app_data_dir));
            let startup_project = std::env::args_os()
                .skip(1)
                .map(PathBuf::from)
                .find(|path| path.is_dir())
                .and_then(|path| Project::open(path).ok().map(|project| project.info()));
            app.manage(StartupProject(startup_project));

            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(CodexProvider::new(processes, approvals));

            let approvals = app.state::<ApprovalBroker>().inner().clone();
            let processes = app.state::<Arc<ProcessRegistry>>().inner().clone();
            app.manage(OpenCodeProvider::new(processes, approvals));

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
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = codex_updates.recv() {
                    if handle.emit("codex-event", event).is_err() {
                        break;
                    }
                }
            });

            let opencode_updates = app.state::<OpenCodeProvider>().subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                while let Ok(event) = opencode_updates.recv() {
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
            git_repository_status,
            git_file_diff,
            git_file_versions,
            list_project_directory,
            read_project_file,
            restore_workspace,
            remember_project,
            close_project_record,
            set_active_project,
            set_project_provider,
            select_project_session,
            sync_provider_sessions,
            pending_approvals,
            resolve_approval,
            codex_list_sessions,
            codex_start_session,
            codex_resume_session,
            codex_send_prompt,
            codex_cancel_turn,
            opencode_list_sessions,
            opencode_start_session,
            opencode_resume_session,
            opencode_session_history,
            opencode_send_prompt,
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

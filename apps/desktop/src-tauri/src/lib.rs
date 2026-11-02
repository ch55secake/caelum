#![forbid(unsafe_code)]

mod commands;
mod events;
mod state;

use crate::state::{PersistenceState, StartupProject};
use caelum_core::approvals::ApprovalBroker;
use caelum_core::codex::CodexProvider;
use caelum_core::opencode::OpenCodeProvider;
use caelum_core::processes::ProcessRegistry;
use caelum_core::projects::Project;
use caelum_core::questions::QuestionBroker;
use caelum_core::session_branches::SessionBranches;
use caelum_core::worktrees::WorktreeClient;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

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

            crate::events::forward(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::opencode::opencode_stats_projects,
            commands::opencode::opencode_stats,
            commands::opencode::opencode_generate,
            commands::projects::app_info,
            commands::projects::startup_project,
            commands::projects::open_project,
            commands::worktrees::git_worktrees,
            commands::worktrees::worktree_destination,
            commands::worktrees::create_worktree,
            commands::worktrees::open_worktree,
            commands::worktrees::remove_worktree,
            commands::git::git_repository_status,
            commands::github::github_open_pull_request,
            commands::github::github_pull_request_details,
            commands::github::github_pull_request_files,
            commands::github::github_pull_request_feedback,
            commands::github::github_merge_pull_request,
            commands::github::github_update_pull_request_branch,
            commands::git::git_file_diff,
            commands::git::git_file_versions,
            commands::files::list_project_directory,
            commands::files::read_project_file,
            commands::files::read_project_image,
            commands::files::opencode_read_image,
            commands::files::search_project_files,
            commands::workspace::restore_workspace,
            commands::workspace::remember_project,
            commands::workspace::close_project_record,
            commands::workspace::set_active_project,
            commands::workspace::set_project_provider,
            commands::workspace::select_project_session,
            commands::workspace::sync_provider_sessions,
            commands::workspace::forget_project_session,
            commands::sessions::session_details,
            commands::sessions::set_session_archived,
            commands::requests::pending_approvals,
            commands::requests::resolve_approval,
            commands::requests::pending_questions,
            commands::requests::refresh_questions,
            commands::requests::resolve_question,
            commands::codex::codex_list_sessions,
            commands::codex::codex_start_session,
            commands::sessions::provider_choices,
            commands::codex::codex_resume_session,
            commands::codex::codex_session_history,
            commands::codex::codex_send_prompt,
            commands::codex::codex_cancel_turn,
            commands::opencode::opencode_list_sessions,
            commands::opencode::opencode_start_session,
            commands::opencode::opencode_resume_session,
            commands::opencode::opencode_session_history,
            commands::opencode::opencode_send_prompt,
            commands::opencode::opencode_enqueue_prompt,
            commands::opencode::opencode_inbox,
            commands::opencode::opencode_cancel_queued_prompt,
            commands::opencode::opencode_select_agent,
            commands::opencode::opencode_commands,
            commands::opencode::opencode_run_command,
            commands::opencode::opencode_usage_limits,
            commands::opencode::opencode_cancel_turn
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

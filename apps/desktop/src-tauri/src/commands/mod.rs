mod about;
mod files;
mod git;
mod github;
mod projects;
mod providers;
mod requests;
mod workspace;
mod worktrees;

// The only command registry. Wire names remain independent of Rust module paths.
pub(crate) fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        about::app_info,
        about::check_for_updates,
        projects::startup_project,
        projects::open_project,
        worktrees::git_worktrees,
        worktrees::worktree_destination,
        worktrees::create_worktree,
        worktrees::open_worktree,
        worktrees::remove_worktree,
        git::git_repository_status,
        git::git_file_diff,
        git::git_file_versions,
        github::github_open_pull_request,
        github::github_pull_request_details,
        github::github_pull_request_files,
        github::github_pull_request_feedback,
        github::github_merge_pull_request,
        github::github_update_pull_request_branch,
        files::list_project_directory,
        files::read_project_file,
        files::read_project_image,
        files::opencode_read_image,
        files::search_project_files,
        workspace::restore_workspace,
        workspace::remember_project,
        workspace::close_project_record,
        workspace::set_active_project,
        workspace::set_project_provider,
        workspace::select_project_session,
        workspace::sync_provider_sessions,
        workspace::forget_project_session,
        requests::pending_approvals,
        requests::resolve_approval,
        requests::pending_questions,
        requests::refresh_questions,
        requests::resolve_question,
        providers::sessions::session_details,
        providers::sessions::set_session_archived,
        providers::sessions::provider_choices,
        providers::codex::codex_list_sessions,
        providers::codex::codex_start_session,
        providers::codex::codex_resume_session,
        providers::codex::codex_session_history,
        providers::codex::codex_send_prompt,
        providers::codex::codex_cancel_turn,
        providers::opencode::sessions::opencode_list_sessions,
        providers::opencode::sessions::opencode_start_session,
        providers::opencode::sessions::opencode_resume_session,
        providers::opencode::sessions::opencode_session_history,
        providers::opencode::conversation::opencode_generate,
        providers::opencode::conversation::opencode_send_prompt,
        providers::opencode::conversation::opencode_enqueue_prompt,
        providers::opencode::conversation::opencode_inbox,
        providers::opencode::conversation::opencode_cancel_queued_prompt,
        providers::opencode::conversation::opencode_select_agent,
        providers::opencode::conversation::opencode_commands,
        providers::opencode::conversation::opencode_run_command,
        providers::opencode::conversation::opencode_cancel_turn,
        providers::opencode::usage::opencode_stats_projects,
        providers::opencode::usage::opencode_stats,
        providers::opencode::usage::opencode_usage_limits,
    ])
}

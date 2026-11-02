use caelum_core::github;
use caelum_protocol::{
    GitHubFeedbackKind, GitHubMergeMethod, GitHubPullRequest, GitHubPullRequestDetails,
    GitHubPullRequestFeedback, GitHubPullRequestFiles,
};

#[tauri::command]
pub(crate) async fn github_open_pull_request(
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
pub(crate) async fn github_pull_request_details(
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
pub(crate) async fn github_pull_request_files(
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
pub(crate) async fn github_pull_request_feedback(
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
pub(crate) async fn github_merge_pull_request(
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
pub(crate) async fn github_update_pull_request_branch(
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

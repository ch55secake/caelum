use super::*;
use caelum_protocol::GitHubAutoMergeResult;
use serde_json::Value;

/// `Some(method)` enables auto-merge; `None` disables it.
pub fn set_pull_request_auto_merge(
    project_path: impl AsRef<Path>,
    url: &str,
    head_oid: &str,
    method: Option<GitHubMergeMethod>,
) -> Result<GitHubAutoMergeResult, GitHubError> {
    set_auto_merge(
        &gh_executable(),
        project_path.as_ref(),
        url,
        head_oid,
        method,
    )
}

fn set_auto_merge(
    executable: &Path,
    project_path: &Path,
    url: &str,
    head_oid: &str,
    method: Option<GitHubMergeMethod>,
) -> Result<GitHubAutoMergeResult, GitHubError> {
    let parsed = pull_request_url(url)?;
    validate_head(head_oid)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    let project = Project::open(project_path)?;
    let command = || {
        let mut command = github_command(executable, project.path());
        command.args(["api", "--hostname", parsed.host_str().unwrap(), "graphql"]);
        command
    };
    let mut lookup = command();
    lookup.args([
        "-f", &format!("owner={}", segments[0]),
        "-f", &format!("repo={}", segments[1]),
        "-F", &format!("number={}", segments[3]),
        "-f", "query=query($owner: String!, $repo: String!, $number: Int!) { repository(owner: $owner, name: $repo) { pullRequest(number: $number) { id url headRefOid state isDraft } } }",
    ]);
    let response = graphql(lookup)?;
    let pr = &response["data"]["repository"]["pullRequest"];
    if pr["url"].as_str() != Some(url) {
        return Err(GitHubError::Command(
            "GitHub returned a different pull request".into(),
        ));
    }
    if pr["headRefOid"].as_str() != Some(head_oid) {
        return Err(GitHubError::Command(
            "The pull request head changed. Refresh the pull request and try again.".into(),
        ));
    }
    if pr["state"] != "OPEN" || (method.is_some() && pr["isDraft"] != false) {
        return Err(GitHubError::Command(
            "Auto-merge requires an open pull request ready for review.".into(),
        ));
    }
    let id = pr["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| GitHubError::Command("Missing pull request ID".into()))?;
    let mut mutation = command();
    mutation.args(["-f", &format!("id={id}")]);
    let (operation, variables, input) = if let Some(method) = method {
        let method = match method {
            GitHubMergeMethod::Merge => "MERGE",
            GitHubMergeMethod::Squash => "SQUASH",
            GitHubMergeMethod::Rebase => "REBASE",
        };
        mutation.args([
            "-f",
            &format!("method={method}"),
            "-f",
            &format!("head={head_oid}"),
        ]);
        (
            "enablePullRequestAutoMerge",
            "$id: ID!, $method: PullRequestMergeMethod!, $head: GitObjectID!",
            "pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $head",
        )
    } else {
        (
            "disablePullRequestAutoMerge",
            "$id: ID!",
            "pullRequestId: $id",
        )
    };
    // Use the native setting mutation: `gh pr merge --auto` can instead merge
    // immediately. GitHub enforces repository permissions and branch rules.
    mutation.args(["-f", &format!("query=mutation({variables}) {{ {operation}(input: {{ {input} }}) {{ pullRequest {{ url headRefOid state autoMergeRequest {{ mergeMethod }} }} }} }}")]);
    let response = graphql(mutation)?;
    let pr = &response["data"][operation]["pullRequest"];
    if pr["url"].as_str() != Some(url) || pr.get("autoMergeRequest").is_none() {
        return Err(GitHubError::Command(
            "GitHub did not confirm the auto-merge change. Refresh the pull request.".into(),
        ));
    }
    let result: GitHubAutoMergeResult = serde_json::from_value(pr.clone())?;
    if result.state != "MERGED"
        && (result.state != "OPEN"
            || result
                .auto_merge_request
                .as_ref()
                .map(|request| request.merge_method)
                != method)
    {
        return Err(GitHubError::Command(
            "GitHub did not confirm the auto-merge change. Refresh the pull request.".into(),
        ));
    }
    Ok(result)
}

fn graphql(command: Command) -> Result<Value, GitHubError> {
    let output = checked_output(command, Duration::from_secs(30))?;
    let response: Value = serde_json::from_slice(&output.stdout)?;
    if let Some(errors) = response
        .get("errors")
        .filter(|errors| errors.as_array().is_none_or(|errors| !errors.is_empty()))
    {
        return Err(GitHubError::Command(format!(
            "Could not change auto-merge: {errors}"
        )));
    }
    Ok(response)
}

#[cfg(all(test, unix))]
#[path = "auto_merge_tests.rs"]
mod tests;

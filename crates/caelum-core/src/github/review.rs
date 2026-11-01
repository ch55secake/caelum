use super::*;
use caelum_protocol::{
    GitHubFeedbackKind, GitHubPullRequestFeedback, GitHubPullRequestFile, GitHubPullRequestFiles,
};
use serde_json::{json, Value};

const PAGE_SIZE: usize = 30;
const COMMENT_FIELDS: &str = "id body url createdAt author { login }";
const PAGE_INFO: &str = "pageInfo { hasNextPage endCursor }";

pub fn pull_request_files(
    project_path: impl AsRef<Path>,
    url: &str,
    head_oid: &str,
    page: u32,
) -> Result<GitHubPullRequestFiles, GitHubError> {
    files(&gh_executable(), project_path.as_ref(), url, head_oid, page)
}

fn files(
    executable: &Path,
    project_path: &Path,
    url: &str,
    head_oid: &str,
    page: u32,
) -> Result<GitHubPullRequestFiles, GitHubError> {
    validate_head(head_oid)?;
    if !(1..=100).contains(&page) {
        return Err(GitHubError::Command(
            "Invalid pull request files page".into(),
        ));
    }
    let parsed = pull_request_url(url)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    let project = Project::open(project_path)?;
    let endpoint = format!(
        "repos/{}/{}/pulls/{}",
        segments[0], segments[1], segments[3]
    );
    let api = |endpoint: &str| -> Result<Value, GitHubError> {
        let mut command = github_command(executable, project.path());
        command.args([
            "api",
            "--hostname",
            parsed.host_str().ok_or(GitHubError::InvalidUrl)?,
            "--method",
            "GET",
            endpoint,
        ]);
        let output = checked_output(command, Duration::from_secs(10))?;
        Ok(serde_json::from_slice(&output.stdout)?)
    };
    // Files are read from the canonical PR, never the local working-tree diff.
    // Check both ends of a page read so changed heads cannot mix paginated diffs.
    let before = api(&endpoint)?;
    check_target(&before, url, head_oid, true)?;
    let files: Vec<GitHubPullRequestFile> = serde_json::from_value(api(&format!(
        "{endpoint}/files?per_page={PAGE_SIZE}&page={page}"
    ))?)?;
    let after = api(&endpoint)?;
    check_target(&after, url, head_oid, true)?;
    if before.pointer("/base/sha") != after.pointer("/base/sha") {
        return Err(GitHubError::Command(
            "The pull request base changed. Refresh the pull request and try again.".into(),
        ));
    }
    Ok(GitHubPullRequestFiles {
        base_oid: before
            .pointer("/base/sha")
            .and_then(Value::as_str)
            .ok_or_else(|| GitHubError::Command("Missing pull request base commit".into()))?
            .to_owned(),
        next_page: (files.len() == PAGE_SIZE && page < 100).then_some(page + 1),
        files,
    })
}

pub fn pull_request_feedback(
    project_path: impl AsRef<Path>,
    url: &str,
    head_oid: &str,
    kind: GitHubFeedbackKind,
    cursor: Option<&str>,
    thread_id: Option<&str>,
) -> Result<GitHubPullRequestFeedback, GitHubError> {
    feedback(
        &gh_executable(),
        project_path.as_ref(),
        url,
        head_oid,
        kind,
        cursor,
        thread_id,
    )
}

fn feedback(
    executable: &Path,
    project_path: &Path,
    url: &str,
    head_oid: &str,
    kind: GitHubFeedbackKind,
    cursor: Option<&str>,
    thread_id: Option<&str>,
) -> Result<GitHubPullRequestFeedback, GitHubError> {
    validate_head(head_oid)?;
    let parsed = pull_request_url(url)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    let project = Project::open(project_path)?;
    let selection = match kind {
        GitHubFeedbackKind::Discussion | GitHubFeedbackKind::Replies => format!("comments(first: {PAGE_SIZE}, after: $cursor) {{ nodes {{ {COMMENT_FIELDS} }} {PAGE_INFO} }}"),
        GitHubFeedbackKind::Reviews => format!("reviews(first: {PAGE_SIZE}, after: $cursor) {{ nodes {{ {COMMENT_FIELDS} state }} {PAGE_INFO} }}"),
        GitHubFeedbackKind::Threads => format!("reviewThreads(first: {PAGE_SIZE}, after: $cursor) {{ nodes {{ id path line startLine originalLine diffSide isResolved isOutdated context: comments(first: 1) {{ nodes {{ diffHunk }} }} comments(first: {PAGE_SIZE}) {{ nodes {{ {COMMENT_FIELDS} }} {PAGE_INFO} }} }} {PAGE_INFO} }}"),
    };
    let mut command = github_command(executable, project.path());
    command.args([
        "api",
        "--hostname",
        parsed.host_str().ok_or(GitHubError::InvalidUrl)?,
        "graphql",
    ]);
    let query = if kind == GitHubFeedbackKind::Replies {
        let id = thread_id
            .filter(|id| !id.is_empty())
            .ok_or_else(|| GitHubError::Command("Missing review thread ID".into()))?;
        command.args(["-f", &format!("id={id}")]);
        format!("query($id: ID!, $cursor: String) {{ node(id: $id) {{ ... on PullRequestReviewThread {{ pullRequest {{ url headRefOid }} {selection} }} }} }}")
    } else {
        command.args([
            "-f",
            &format!("owner={}", segments[0]),
            "-f",
            &format!("repo={}", segments[1]),
            "-F",
            &format!("number={}", segments[3]),
        ]);
        format!("query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {{ repository(owner: $owner, name: $repo) {{ pullRequest(number: $number) {{ url headRefOid {selection} }} }} }}")
    };
    command.args(["-f", &format!("query={query}")]);
    if let Some(cursor) = cursor {
        command.args(["-f", &format!("cursor={cursor}")]);
    }
    let output = checked_output(command, Duration::from_secs(15))?;
    let response: Value = serde_json::from_slice(&output.stdout)?;
    if response
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
    {
        return Err(GitHubError::Command(format!(
            "Could not load review feedback: {}",
            response["errors"]
        )));
    }
    let node = response
        .pointer(if kind == GitHubFeedbackKind::Replies {
            "/data/node"
        } else {
            "/data/repository/pullRequest"
        })
        .ok_or_else(|| GitHubError::Command("Pull request feedback is unavailable".into()))?;
    check_target(
        if kind == GitHubFeedbackKind::Replies {
            &node["pullRequest"]
        } else {
            node
        },
        url,
        head_oid,
        false,
    )?;
    let connection = &node[match kind {
        GitHubFeedbackKind::Discussion | GitHubFeedbackKind::Replies => "comments",
        GitHubFeedbackKind::Reviews => "reviews",
        GitHubFeedbackKind::Threads => "reviewThreads",
    }];
    let mut nodes = connection
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| GitHubError::Command("Invalid pull request feedback response".into()))?;
    let cursor = next_cursor(connection)?;
    let (comments, threads) = if kind == GitHubFeedbackKind::Threads {
        for thread in &mut nodes {
            let next = next_cursor(&thread["comments"])?;
            let mut comments = thread["comments"]["nodes"].take();
            if let Some(first) = comments.as_array_mut().and_then(|items| items.first_mut()) {
                first["diffHunk"] = thread
                    .pointer("/context/nodes/0/diffHunk")
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            thread["comments"] = comments;
            thread["nextCursor"] = json!(next);
        }
        (vec![], serde_json::from_value(json!(nodes))?)
    } else {
        (serde_json::from_value(json!(nodes))?, vec![])
    };
    Ok(GitHubPullRequestFeedback {
        comments,
        threads,
        next_cursor: cursor,
    })
}

fn next_cursor(connection: &Value) -> Result<Option<String>, GitHubError> {
    match connection
        .pointer("/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
    {
        Some(false) => Ok(None),
        Some(true) => connection
            .pointer("/pageInfo/endCursor")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| GitHubError::Command("Missing review pagination cursor".into())),
        None => Err(GitHubError::Command(
            "Invalid review pagination response".into(),
        )),
    }
}

fn check_target(value: &Value, url: &str, head: &str, rest: bool) -> Result<(), GitHubError> {
    if value
        .get(if rest { "html_url" } else { "url" })
        .and_then(Value::as_str)
        != Some(url)
    {
        return Err(GitHubError::Command(
            "GitHub returned a different pull request".into(),
        ));
    }
    if value
        .pointer(if rest { "/head/sha" } else { "/headRefOid" })
        .and_then(Value::as_str)
        != Some(head)
    {
        return Err(GitHubError::Command(
            "The pull request head changed. Refresh the pull request and try again.".into(),
        ));
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "review_tests.rs"]
mod tests;

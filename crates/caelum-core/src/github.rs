use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use caelum_protocol::{
    GitHubMergeMethod, GitHubMergeResult, GitHubPullRequest, GitHubPullRequestDetails,
};
use thiserror::Error;

use crate::projects::{Project, ProjectError};

/// Uses gh's existing authentication and repository/remote selection. Network
/// lookups stay separate from local Git status and never open an auth prompt.
pub fn open_pull_request(
    project_path: impl AsRef<Path>,
    branch: &str,
) -> Result<Option<GitHubPullRequest>, GitHubError> {
    query_pull_request(
        &gh_executable(),
        project_path,
        branch,
        Duration::from_secs(10),
    )
}

pub fn pull_request_details(
    project_path: impl AsRef<Path>,
    url: &str,
) -> Result<GitHubPullRequestDetails, GitHubError> {
    query_details(&gh_executable(), project_path, url, Duration::from_secs(10))
}

fn query_details(
    executable: &Path,
    project_path: impl AsRef<Path>,
    url: &str,
    timeout: Duration,
) -> Result<GitHubPullRequestDetails, GitHubError> {
    // Use the URL returned by discovery, so forks and different worktrees cannot
    // accidentally resolve the same PR number in a different repository.
    pull_request_url(url)?;
    let project = Project::open(project_path)?;
    let mut command = github_command(executable, project.path());
    command.args(["pr", "view", url, "--json", "number,title,url,isDraft,body,state,author,baseRefName,headRefName,headRefOid,mergeable,mergeStateStatus,additions,deletions,changedFiles,reviewDecision,statusCheckRollup"]);
    let output = checked_output(command, timeout)?;
    let details: GitHubPullRequestDetails = serde_json::from_slice(&output.stdout)?;
    if details.pull_request.url != url {
        return Err(GitHubError::Command(
            "GitHub returned a different pull request".into(),
        ));
    }
    Ok(details)
}

pub fn merge_pull_request(
    project_path: impl AsRef<Path>,
    url: &str,
    method: GitHubMergeMethod,
    head_oid: &str,
) -> Result<(), GitHubError> {
    merge_request(
        &gh_executable(),
        project_path,
        url,
        method,
        head_oid,
        Duration::from_secs(30),
    )
}

fn merge_request(
    executable: &Path,
    project_path: impl AsRef<Path>,
    url: &str,
    method: GitHubMergeMethod,
    head_oid: &str,
    timeout: Duration,
) -> Result<(), GitHubError> {
    let parsed = pull_request_url(url)?;
    validate_head(head_oid)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    let project = Project::open(project_path)?;
    let mut command = github_command(executable, project.path());
    let method = match method {
        GitHubMergeMethod::Merge => "merge",
        GitHubMergeMethod::Squash => "squash",
        GitHubMergeMethod::Rebase => "rebase",
    };
    // The API confirms whether a merge actually happened and enforces repository
    // rules. Pin it to the displayed head, including when the branch is updated.
    command.args([
        "api",
        "--hostname",
        parsed.host_str().ok_or(GitHubError::InvalidUrl)?,
        "--method",
        "PUT",
        &format!(
            "repos/{}/{}/pulls/{}/merge",
            segments[0], segments[1], segments[3]
        ),
        "-f",
        &format!("merge_method={method}"),
        "-f",
        &format!("sha={head_oid}"),
    ]);
    let output = checked_output(command, timeout)?;
    let response: GitHubMergeResult = serde_json::from_slice(&output.stdout)?;
    if !response.merged {
        return Err(GitHubError::Command(response.message));
    }
    Ok(())
}

pub fn update_pull_request_branch(
    project_path: impl AsRef<Path>,
    url: &str,
    head_oid: &str,
) -> Result<(), GitHubError> {
    update_branch_request(
        &gh_executable(),
        project_path,
        url,
        head_oid,
        Duration::from_secs(30),
    )
}

fn update_branch_request(
    executable: &Path,
    project_path: impl AsRef<Path>,
    url: &str,
    head_oid: &str,
    timeout: Duration,
) -> Result<(), GitHubError> {
    let parsed = pull_request_url(url)?;
    validate_head(head_oid)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    let project = Project::open(project_path)?;
    let mut command = github_command(executable, project.path());
    command.args([
        "api",
        "--hostname",
        parsed.host_str().ok_or(GitHubError::InvalidUrl)?,
        "--method",
        "PUT",
        &format!(
            "repos/{}/{}/pulls/{}/update-branch",
            segments[0], segments[1], segments[3]
        ),
        "-f",
        &format!("expected_head_sha={head_oid}"),
    ]);
    // GitHub merges the latest base into the remote PR branch asynchronously.
    // A successful request is only acceptance; the UI waits for a new head.
    checked_output(command, timeout)?;
    Ok(())
}

fn validate_head(head_oid: &str) -> Result<(), GitHubError> {
    if !matches!(head_oid.len(), 40 | 64) || !head_oid.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(GitHubError::InvalidHead);
    }
    Ok(())
}

fn pull_request_url(url: &str) -> Result<reqwest::Url, GitHubError> {
    let parsed = reqwest::Url::parse(url).map_err(|_| GitHubError::InvalidUrl)?;
    let segments: Vec<_> = parsed
        .path_segments()
        .ok_or(GitHubError::InvalidUrl)?
        .collect();
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || segments.len() != 4
        || segments[0].is_empty()
        || segments[1].is_empty()
        || segments[2] != "pull"
        || segments[3].parse::<u64>().unwrap_or(0) == 0
    {
        return Err(GitHubError::InvalidUrl);
    }
    Ok(parsed)
}

fn query_pull_request(
    executable: &Path,
    project_path: impl AsRef<Path>,
    branch: &str,
    timeout: Duration,
) -> Result<Option<GitHubPullRequest>, GitHubError> {
    let project = Project::open(project_path)?;
    if branch.is_empty() {
        return Ok(None);
    }
    let mut command = github_command(executable, project.path());
    command
        .args(["pr", "list", "--state=open", "--limit=1"])
        .arg(format!("--head={branch}"))
        .args(["--json", "number,title,url,isDraft"]);
    let output = checked_output(command, timeout)?;
    let requests: Vec<GitHubPullRequest> = serde_json::from_slice(&output.stdout)?;
    Ok(requests.into_iter().next())
}

fn github_command(executable: &Path, project_path: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(project_path)
        .env_remove("GH_REPO")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "cat")
        .env("GH_SPINNER_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1");
    command
}

fn checked_output(command: Command, timeout: Duration) -> Result<Output, GitHubError> {
    let output = bounded_output(command, timeout)?;
    if !output.status.success() {
        return Err(GitHubError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(output)
}

fn gh_executable() -> PathBuf {
    let name = if cfg!(windows) { "gh.exe" } else { "gh" };
    let mut candidates: Vec<PathBuf> = env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths)
                .map(|path| path.join(name))
                .collect()
        })
        .unwrap_or_default();
    if let Some(home) = env::var_os("HOME") {
        for directory in [".local/bin", ".nix-profile/bin"] {
            candidates.push(PathBuf::from(&home).join(directory).join(name));
        }
    }
    for directory in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/run/current-system/sw/bin",
    ] {
        candidates.push(PathBuf::from(directory).join(name));
    }
    candidates
        .into_iter()
        .find(|path| {
            let Ok(metadata) = fs::metadata(path) else {
                return false;
            };
            if !metadata.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        })
        .unwrap_or_else(|| PathBuf::from(name))
}

fn bounded_output(mut command: Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    fn read(pipe: impl Read) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        pipe.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GitHub response exceeds 1 MiB",
            ));
        }
        Ok(bytes)
    }
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout = thread::spawn(move || read(stdout));
    let stderr = thread::spawn(move || read(stderr));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                break Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "GitHub request timed out",
                ))
            }
            Err(error) => break Err(error),
        }
    };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = stdout
        .join()
        .map_err(|_| io::Error::other("GitHub stdout reader failed"))?;
    let stderr = stderr
        .join()
        .map_err(|_| io::Error::other("GitHub stderr reader failed"))?;
    Ok(Output {
        status: status?,
        stdout: stdout?,
        stderr: stderr?,
    })
}

#[derive(Debug, Error)]
pub enum GitHubError {
    #[error("Invalid GitHub pull request URL")]
    InvalidUrl,
    #[error("Invalid pull request head commit; refresh the pull request and try again")]
    InvalidHead,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("GitHub CLI is unavailable: {0}")]
    Io(#[from] io::Error),
    #[error("GitHub request failed: {0}")]
    Command(String),
    #[error("Invalid GitHub pull request response: {0}")]
    Response(#[from] serde_json::Error),
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(script: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = env::temp_dir().join(format!(
                "caelum-gh-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            let executable = path.join("gh");
            fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn query(&self, branch: &str) -> Result<Option<GitHubPullRequest>, GitHubError> {
            query_pull_request(&self.0.join("gh"), &self.0, branch, Duration::from_secs(2))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn queries_only_open_prs_in_the_project_with_literal_branch_arguments() {
        let fixture = Fixture::new(
            r#"
printf '%s\n' "$@" > arguments
pwd > directory
printf '%s' "$GH_PROMPT_DISABLED" > no-prompts
printf '%s' '[{"number":40,"title":"Session UI","url":"https://github.com/example/repo/pull/40","isDraft":true}]'
"#,
        );
        let pr = fixture.query("feat/$(touch unexpected)").unwrap().unwrap();
        assert_eq!(pr.number, 40);
        assert!(pr.is_draft);
        assert_eq!(fs::read_to_string(fixture.0.join("arguments")).unwrap(), "pr\nlist\n--state=open\n--limit=1\n--head=feat/$(touch unexpected)\n--json\nnumber,title,url,isDraft\n");
        assert_eq!(
            fs::read_to_string(fixture.0.join("directory"))
                .unwrap()
                .trim(),
            fixture.0.to_str().unwrap()
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("no-prompts")).unwrap(),
            "1"
        );
        assert!(!fixture.0.join("unexpected").exists());
    }

    #[test]
    fn empty_results_are_distinct_from_unavailable_or_invalid_lookups() {
        let empty = Fixture::new("printf '[]'");
        assert_eq!(empty.query("main").unwrap(), None);
        assert_eq!(empty.query("").unwrap(), None);
        let failure = Fixture::new("printf 'Authentication required' >&2\nexit 1");
        assert!(matches!(
            failure.query("main"),
            Err(GitHubError::Command(_))
        ));
        let invalid = Fixture::new("printf 'invalid'");
        assert!(matches!(
            invalid.query("main"),
            Err(GitHubError::Response(_))
        ));
        assert!(matches!(
            query_pull_request(
                &empty.0.join("missing"),
                &empty.0,
                "main",
                Duration::from_secs(1)
            ),
            Err(GitHubError::Io(_))
        ));
    }

    #[test]
    fn hung_github_processes_are_terminated_and_reaped() {
        let fixture = Fixture::new("echo $$ > pid\nexec sleep 30");
        let start = Instant::now();
        let result = query_pull_request(
            &fixture.0.join("gh"),
            &fixture.0,
            "main",
            Duration::from_secs(3),
        );
        assert!(
            matches!(result, Err(GitHubError::Io(error)) if error.kind() == io::ErrorKind::TimedOut)
        );
        assert!(start.elapsed() < Duration::from_secs(8));
        let pid = fs::read_to_string(fixture.0.join("pid")).unwrap();
        assert!(!Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }

    fn details_response(url: &str) -> String {
        serde_json::json!({
            "number": 43, "title": "Worktrees", "url": url, "isDraft": false,
            "body": "## Description\nIndependent checkouts", "state": "MERGED",
            "author": { "login": "contributor" }, "headRefName": "feature/worktrees",
            "baseRefName": "main", "additions": 128, "deletions": 42, "changedFiles": 6,
            "headRefOid": "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
            "mergeable": "MERGEABLE", "mergeStateStatus": "CLEAN",
            "reviewDecision": "APPROVED",
            "statusCheckRollup": [
                { "__typename": "CheckRun", "name": "CI", "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": "https://github.com/example/repo/actions/runs/1", "startedAt": "2026-11-01T12:00:00Z", "completedAt": "2026-11-01T12:02:14Z" },
                { "__typename": "StatusContext", "context": "Deploy", "state": "PENDING", "targetUrl": "https://deploy.example.com/1" }
            ]
        }).to_string()
    }

    #[test]
    fn details_use_the_canonical_url_and_support_both_check_types() {
        let url = "https://git.example.com/upstream/repo/pull/43";
        let fixture = Fixture::new(&format!(
            "printf '%s\\n' \"$@\" > arguments\npwd > directory\nprintf '%s' '{}'",
            details_response(url)
        ));
        let result = query_details(
            &fixture.0.join("gh"),
            &fixture.0,
            url,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(result.state, "MERGED");
        assert_eq!(result.additions, 128);
        assert_eq!(result.deletions, 42);
        assert_eq!(result.changed_files, 6);
        assert_eq!(result.merge_state_status, "CLEAN");
        assert_eq!(
            result.head_ref_oid,
            "abcdefabcdefabcdefabcdefabcdefabcdefabcd"
        );
        let serialized = serde_json::to_value(&result).unwrap();
        assert_eq!(
            serialized["statusCheckRollup"][0]["detailsUrl"],
            "https://github.com/example/repo/actions/runs/1"
        );
        assert_eq!(
            serialized["statusCheckRollup"][1]["targetUrl"],
            "https://deploy.example.com/1"
        );
        assert_eq!(
            serialized["statusCheckRollup"][0]["startedAt"],
            "2026-11-01T12:00:00Z"
        );
        assert_eq!(
            serialized["statusCheckRollup"][0]["completedAt"],
            "2026-11-01T12:02:14Z"
        );
        assert_eq!(result.author.unwrap().login, "contributor");
        assert_eq!(result.status_check_rollup.unwrap().len(), 2);
        let args = fs::read_to_string(fixture.0.join("arguments")).unwrap();
        assert!(args.starts_with(&format!("pr\nview\n{url}\n--json\n")));
        assert!(args.contains("statusCheckRollup"));
        assert_eq!(
            fs::read_to_string(fixture.0.join("directory"))
                .unwrap()
                .trim(),
            fixture.0.to_str().unwrap()
        );
    }

    #[test]
    fn details_reject_invalid_targets_and_mismatched_repositories() {
        let fixture = Fixture::new(&format!(
            "printf '%s' '{}'",
            details_response("https://github.com/other/repo/pull/43")
        ));
        for url in [
            "--help",
            "43",
            "http://github.com/owner/repo/pull/43",
            "https://github.com/owner/repo/issues/43",
            "https://github.com/owner/repo/pull/0",
            "https://github.com/owner/repo/pull/43?x=1",
        ] {
            assert!(matches!(
                query_details(
                    &fixture.0.join("missing"),
                    &fixture.0,
                    url,
                    Duration::from_secs(1)
                ),
                Err(GitHubError::InvalidUrl)
            ));
        }
        assert!(matches!(
            query_details(
                &fixture.0.join("gh"),
                &fixture.0,
                "https://github.com/owner/repo/pull/43",
                Duration::from_secs(2)
            ),
            Err(GitHubError::Command(_))
        ));
    }

    #[test]
    fn details_preserve_missing_metadata_and_report_failures() {
        let missing: caelum_protocol::GitHubStatusCheck =
            serde_json::from_value(serde_json::json!({
                "__typename": "CheckRun", "name": "Queued", "status": "QUEUED", "completedAt": null
            }))
            .unwrap();
        assert!(matches!(
            missing,
            caelum_protocol::GitHubStatusCheck::CheckRun {
                started_at: None,
                completed_at: None,
                ..
            }
        ));
        let url = "https://github.com/owner/repo/pull/43";
        let mut response: serde_json::Value = serde_json::from_str(&details_response(url)).unwrap();
        response["author"] = serde_json::Value::Null;
        response["statusCheckRollup"] = serde_json::Value::Null;
        response["reviewDecision"] = serde_json::Value::Null;
        let fixture = Fixture::new(&format!("printf '%s' '{response}'"));
        let details = query_details(
            &fixture.0.join("gh"),
            &fixture.0,
            url,
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(details.author.is_none());
        assert!(details.status_check_rollup.is_none());
        let failed = Fixture::new("printf 'Authentication required' >&2\nexit 1");
        assert!(
            matches!(query_details(&failed.0.join("gh"), &failed.0, url, Duration::from_secs(2)), Err(GitHubError::Command(message)) if message.contains("Authentication required"))
        );
        let invalid = Fixture::new("printf '{}'");
        assert!(matches!(
            query_details(
                &invalid.0.join("gh"),
                &invalid.0,
                url,
                Duration::from_secs(2)
            ),
            Err(GitHubError::Response(_))
        ));
    }

    #[test]
    fn oversized_github_responses_are_reported_explicitly() {
        let fixture = Fixture::new("head -c 1100000 /dev/zero");
        assert!(
            matches!(fixture.query("main"), Err(GitHubError::Io(error)) if error.to_string().contains("exceeds 1 MiB"))
        );
    }

    #[test]
    fn merge_targets_the_canonical_repository_and_exact_head_for_each_method() {
        let fixture = Fixture::new(
            r#"
printf '%s\n' "$@" > arguments
pwd > directory
printf '%s' "$GH_PROMPT_DISABLED" > no-prompts
printf '%s' '{"merged":true,"message":"Pull Request successfully merged"}'
"#,
        );
        let sha = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
        for (method, name) in [
            (GitHubMergeMethod::Merge, "merge"),
            (GitHubMergeMethod::Squash, "squash"),
            (GitHubMergeMethod::Rebase, "rebase"),
        ] {
            merge_request(
                &fixture.0.join("gh"),
                &fixture.0,
                "https://git.example.com/upstream/repo/pull/43",
                method,
                sha,
                Duration::from_secs(2),
            )
            .unwrap();
            assert_eq!(fs::read_to_string(fixture.0.join("arguments")).unwrap(), format!("api\n--hostname\ngit.example.com\n--method\nPUT\nrepos/upstream/repo/pulls/43/merge\n-f\nmerge_method={name}\n-f\nsha={sha}\n"));
        }
        assert_eq!(
            fs::read_to_string(fixture.0.join("directory"))
                .unwrap()
                .trim(),
            fixture.0.to_str().unwrap()
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("no-prompts")).unwrap(),
            "1"
        );
    }

    #[test]
    fn merge_rejects_invalid_targets_heads_and_methods_before_execution() {
        let fixture = Fixture::new("exit 99");
        let executable = fixture.0.join("missing");
        let url = "https://github.com/owner/repo/pull/43";
        assert!(matches!(
            merge_request(
                &executable,
                &fixture.0,
                "--help",
                GitHubMergeMethod::Merge,
                &"a".repeat(40),
                Duration::from_secs(1)
            ),
            Err(GitHubError::InvalidUrl)
        ));
        for sha in ["", "HEAD", "--help", "$(touch unexpected)", &"z".repeat(40)] {
            assert!(matches!(
                merge_request(
                    &executable,
                    &fixture.0,
                    url,
                    GitHubMergeMethod::Merge,
                    sha,
                    Duration::from_secs(1)
                ),
                Err(GitHubError::InvalidHead)
            ));
        }
        assert!(serde_json::from_str::<GitHubMergeMethod>("\"admin\"").is_err());
        assert!(!fixture.0.join("unexpected").exists());
    }

    #[test]
    fn merge_requires_confirmation_and_preserves_github_rejection_messages() {
        for script in [
            "printf '%s' '{\"merged\":false,\"message\":\"Required checks have not passed\"}'",
            "printf '%s' 'Head branch was modified (HTTP 409)' >&2\nexit 1",
            "printf '%s' 'Merge method is not allowed (HTTP 405)' >&2\nexit 1",
        ] {
            let fixture = Fixture::new(script);
            let result = merge_request(
                &fixture.0.join("gh"),
                &fixture.0,
                "https://github.com/owner/repo/pull/43",
                GitHubMergeMethod::Squash,
                &"a".repeat(40),
                Duration::from_secs(2),
            );
            assert!(matches!(result, Err(GitHubError::Command(message)) if !message.is_empty()));
        }
        let fixture = Fixture::new("printf '{}'");
        assert!(matches!(
            merge_request(
                &fixture.0.join("gh"),
                &fixture.0,
                "https://github.com/owner/repo/pull/43",
                GitHubMergeMethod::Merge,
                &"a".repeat(40),
                Duration::from_secs(2)
            ),
            Err(GitHubError::Response(_))
        ));
    }

    #[test]
    fn branch_update_targets_the_canonical_repository_and_displayed_head() {
        let fixture = Fixture::new(
            r#"
printf '%s\n' "$@" > arguments
pwd > directory
printf '%s' "$GH_PROMPT_DISABLED" > no-prompts
printf '%s' '{"message":"Updating pull request branch.","url":"https://git.example.com/upstream/repo/pull/43"}'
"#,
        );
        let sha = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
        update_branch_request(
            &fixture.0.join("gh"),
            &fixture.0,
            "https://git.example.com/upstream/repo/pull/43",
            sha,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(fixture.0.join("arguments")).unwrap(), format!("api\n--hostname\ngit.example.com\n--method\nPUT\nrepos/upstream/repo/pulls/43/update-branch\n-f\nexpected_head_sha={sha}\n"));
        assert_eq!(
            fs::read_to_string(fixture.0.join("directory"))
                .unwrap()
                .trim(),
            fixture.0.to_str().unwrap()
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("no-prompts")).unwrap(),
            "1"
        );
    }

    #[test]
    fn branch_update_rejects_invalid_targets_and_heads_before_execution() {
        let fixture = Fixture::new("exit 99");
        let executable = fixture.0.join("missing");
        assert!(matches!(
            update_branch_request(
                &executable,
                &fixture.0,
                "--help",
                &"a".repeat(40),
                Duration::from_secs(1)
            ),
            Err(GitHubError::InvalidUrl)
        ));
        for sha in ["", "HEAD", "--help", "$(touch unexpected)", &"z".repeat(40)] {
            assert!(matches!(
                update_branch_request(
                    &executable,
                    &fixture.0,
                    "https://github.com/owner/repo/pull/43",
                    sha,
                    Duration::from_secs(1)
                ),
                Err(GitHubError::InvalidHead)
            ));
        }
        assert!(!fixture.0.join("unexpected").exists());
    }

    #[test]
    fn branch_update_preserves_permission_conflict_and_stale_head_errors() {
        for message in [
            "Write access required (HTTP 403)",
            "Merge conflict (HTTP 422)",
            "Head branch was modified (HTTP 422)",
        ] {
            let fixture = Fixture::new(&format!("printf '%s' '{message}' >&2\nexit 1"));
            let result = update_branch_request(
                &fixture.0.join("gh"),
                &fixture.0,
                "https://github.com/owner/repo/pull/43",
                &"a".repeat(40),
                Duration::from_secs(2),
            );
            assert!(matches!(result, Err(GitHubError::Command(error)) if error == message));
        }
    }
}

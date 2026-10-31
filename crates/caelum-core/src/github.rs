use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use caelum_protocol::GitHubPullRequest;
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
    let mut command = Command::new(executable);
    command
        .current_dir(project.path())
        .args(["pr", "list", "--state=open", "--limit=1"])
        .arg(format!("--head={branch}"))
        .args(["--json", "number,title,url,isDraft"])
        .env_remove("GH_REPO")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "cat")
        .env("GH_SPINNER_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1");
    let output = bounded_output(command, timeout)?;
    if !output.status.success() {
        return Err(GitHubError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let requests: Vec<GitHubPullRequest> = serde_json::from_slice(&output.stdout)?;
    Ok(requests.into_iter().next())
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
        pipe.take(1024 * 1024).read_to_end(&mut bytes)?;
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
                    "GitHub lookup timed out",
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
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("GitHub CLI is unavailable: {0}")]
    Io(#[from] io::Error),
    #[error("GitHub pull request lookup failed: {0}")]
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
            Duration::from_secs(1),
        );
        assert!(
            matches!(result, Err(GitHubError::Io(error)) if error.kind() == io::ErrorKind::TimedOut)
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        let pid = fs::read_to_string(fixture.0.join("pid")).unwrap();
        assert!(!Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }
}

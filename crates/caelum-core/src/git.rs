use std::{
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
};

use caelum_protocol::{GitFileDiff, GitFileStatus, GitRepositoryStatus};
use thiserror::Error;

use crate::projects::{Project, ProjectError};

const PORCELAIN_STATUS_ARGS: &[&str] = &[
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--no-renames",
];

/// Read-only Git inspection for projects opened in Caelum.
#[derive(Clone, Debug)]
pub struct GitClient {
    executable: OsString,
}

impl Default for GitClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GitClient {
    pub fn new() -> Self {
        Self {
            executable: OsString::from("git"),
        }
    }

    #[cfg(test)]
    fn with_executable(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    pub fn repository_status(
        &self,
        project_path: impl AsRef<Path>,
    ) -> Result<GitRepositoryStatus, GitError> {
        let project = Project::open(project_path)?;
        let Some(repository_root) = self.repository_root(&project)? else {
            return Ok(GitRepositoryStatus {
                project_path: project.path().to_string_lossy().into_owned(),
                repository_root: None,
                branch: None,
                changes: Vec::new(),
            });
        };

        let branch_output = self.run(&project, &["branch", "--show-current"])?;
        self.require_success(&branch_output, "read current branch")?;
        let branch = output_text(&branch_output.stdout).trim().to_owned();

        Ok(GitRepositoryStatus {
            project_path: project.path().to_string_lossy().into_owned(),
            repository_root: Some(repository_root.to_string_lossy().into_owned()),
            branch: (!branch.is_empty()).then_some(branch),
            changes: self.project_status_changes(&project, &repository_root)?,
        })
    }

    pub fn file_diff(
        &self,
        project_path: impl AsRef<Path>,
        file_path: &str,
    ) -> Result<GitFileDiff, GitError> {
        let project = Project::open(project_path)?;
        let relative_path = validate_project_relative_path(file_path)?;
        validate_scoped_file(&project, &relative_path)?;

        let repository_root = self
            .repository_root(&project)?
            .ok_or(GitError::NotRepository)?;
        let repository_project = Project::open(&repository_root)?;
        let project_prefix = project_relative_prefix(&project, &repository_root)?;
        let repository_relative_path = project_prefix.join(&relative_path);
        let git_path = git_path_string(&repository_relative_path);
        let changes = self.project_status_changes(&project, &repository_root)?;
        let change = changes
            .iter()
            .find(|change| change.path == file_path)
            .ok_or_else(|| GitError::NotChanged(file_path.to_owned()))?;

        let staged = self.diff_output(
            &repository_project,
            &[
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-textconv",
                "--color=never",
            ],
            &git_path,
        )?;
        let unstaged = self.diff_output(
            &repository_project,
            &["diff", "--no-ext-diff", "--no-textconv", "--color=never"],
            &git_path,
        )?;
        let untracked = if change.index_status == "?" && change.worktree_status == "?" {
            self.untracked_diff(&repository_project, &git_path)?
        } else {
            String::new()
        };

        Ok(GitFileDiff {
            path: file_path.to_owned(),
            staged,
            unstaged,
            untracked,
        })
    }

    fn repository_root(&self, project: &Project) -> Result<Option<PathBuf>, GitError> {
        let output = self.run(project, &["rev-parse", "--show-toplevel"])?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if error.contains("not a git repository") {
                return Ok(None);
            }
            return Err(command_error("detect repository root", error));
        }

        let path = PathBuf::from(output_text(&output.stdout).trim());
        let canonical_root = path
            .canonicalize()
            .map_err(|source| GitError::ResolvePath { path, source })?;
        project_relative_prefix(project, &canonical_root)?;
        Ok(Some(canonical_root))
    }

    fn project_status_changes(
        &self,
        project: &Project,
        repository_root: &Path,
    ) -> Result<Vec<GitFileStatus>, GitError> {
        let repository_project = Project::open(repository_root)?;
        let project_prefix = project_relative_prefix(project, repository_root)?;
        let mut arguments = PORCELAIN_STATUS_ARGS
            .iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        if !project_prefix.as_os_str().is_empty() {
            arguments.push(OsString::from("--"));
            arguments.push(OsString::from(git_path_string(&project_prefix)));
        }
        let output = self.run_os(&repository_project, &arguments)?;
        self.require_success(&output, "read working-tree status")?;
        let mut changes = parse_status(&output.stdout)?;
        for change in &mut changes {
            let repository_relative_path = Path::new(&change.path);
            let project_relative_path = if project_prefix.as_os_str().is_empty() {
                repository_relative_path
            } else {
                repository_relative_path
                    .strip_prefix(&project_prefix)
                    .map_err(|_| GitError::OutsideProject(change.path.clone()))?
            };
            change.path = git_path_string(project_relative_path);
        }
        Ok(changes)
    }

    fn diff_output(
        &self,
        project: &Project,
        args: &[&str],
        file_path: &str,
    ) -> Result<String, GitError> {
        let mut arguments = args.iter().map(OsString::from).collect::<Vec<_>>();
        arguments.push(OsString::from("--"));
        arguments.push(OsString::from(file_path));
        let output = self.run_os(project, &arguments)?;
        self.require_success(&output, "read file diff")?;
        Ok(output_text(&output.stdout))
    }

    fn untracked_diff(&self, project: &Project, file_path: &str) -> Result<String, GitError> {
        let arguments = [
            OsString::from("diff"),
            OsString::from("--no-index"),
            OsString::from("--no-ext-diff"),
            OsString::from("--no-textconv"),
            OsString::from("--color=never"),
            OsString::from("--"),
            null_device_path(),
            OsString::from(file_path),
        ];
        let output = self.run_os(project, &arguments)?;
        if output.status.success() || output.status.code() == Some(1) {
            Ok(output_text(&output.stdout))
        } else {
            Err(command_error(
                "read untracked file diff",
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        }
    }

    fn run(&self, project: &Project, args: &[&str]) -> Result<Output, GitError> {
        let arguments = args.iter().map(OsString::from).collect::<Vec<_>>();
        self.run_os(project, &arguments)
    }

    fn run_os(&self, project: &Project, args: &[OsString]) -> Result<Output, GitError> {
        let mut command = Command::new(&self.executable);
        command
            .arg("-C")
            .arg(project.path())
            .arg("--literal-pathspecs")
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C");
        command.output().map_err(|source| GitError::Unavailable {
            executable: self.executable.clone(),
            source,
        })
    }

    fn require_success(&self, output: &Output, operation: &'static str) -> Result<(), GitError> {
        if output.status.success() {
            Ok(())
        } else {
            Err(command_error(
                operation,
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        }
    }
}

fn parse_status(output: &[u8]) -> Result<Vec<GitFileStatus>, GitError> {
    let mut changes = Vec::new();
    let mut offset = 0;
    while offset < output.len() {
        let end = output[offset..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|position| offset + position)
            .ok_or(GitError::MalformedStatus)?;
        let record = &output[offset..end];
        if record.len() < 4 || record[2] != b' ' {
            return Err(GitError::MalformedStatus);
        }
        changes.push(GitFileStatus {
            index_status: char::from(record[0]).to_string(),
            worktree_status: char::from(record[1]).to_string(),
            path: String::from_utf8_lossy(&record[3..]).into_owned(),
        });
        offset = end + 1;
    }
    Ok(changes)
}

fn project_relative_prefix(project: &Project, repository_root: &Path) -> Result<PathBuf, GitError> {
    project
        .path()
        .strip_prefix(repository_root)
        .map(Path::to_path_buf)
        .map_err(|_| GitError::OutsideProject(project.path().display().to_string()))
}

fn git_path_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn validate_project_relative_path(path: &str) -> Result<PathBuf, GitError> {
    if path.is_empty() || path.contains('\0') {
        return Err(GitError::InvalidPath(path.to_owned()));
    }
    let relative = Path::new(path);
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(GitError::InvalidPath(path.to_owned()));
    }
    Ok(relative.to_path_buf())
}

fn validate_scoped_file(project: &Project, relative_path: &Path) -> Result<(), GitError> {
    let candidate = project.path().join(relative_path);
    match fs::symlink_metadata(&candidate) {
        Ok(_) => {
            let canonical = project.resolve_scoped_path(&candidate)?;
            if !canonical.is_file() && !canonical.is_dir() {
                return Err(GitError::NotAFile(
                    relative_path.to_string_lossy().into_owned(),
                ));
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(GitError::ResolvePath {
                path: candidate,
                source,
            });
        }
    }

    let mut ancestor = candidate.as_path();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or_else(|| {
                    GitError::InvalidPath(relative_path.to_string_lossy().into_owned())
                })?;
            }
            Err(source) => {
                return Err(GitError::ResolvePath {
                    path: ancestor.to_path_buf(),
                    source,
                });
            }
        }
    }
    let canonical_ancestor = ancestor
        .canonicalize()
        .map_err(|source| GitError::ResolvePath {
            path: ancestor.to_path_buf(),
            source,
        })?;
    if !canonical_ancestor.starts_with(project.path()) {
        return Err(GitError::OutsideProject(
            relative_path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

fn output_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn command_error(operation: &'static str, message: String) -> GitError {
    GitError::CommandFailed {
        operation,
        message: if message.is_empty() {
            "Git exited unsuccessfully without an error message".to_owned()
        } else {
            message
        },
    }
}

fn null_device_path() -> OsString {
    #[cfg(windows)]
    {
        OsString::from("NUL")
    }
    #[cfg(not(windows))]
    {
        OsString::from("/dev/null")
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("Git is unavailable at '{executable:?}': {source}")]
    Unavailable {
        executable: OsString,
        #[source]
        source: std::io::Error,
    },
    #[error("Git could not {operation}: {message}")]
    CommandFailed {
        operation: &'static str,
        message: String,
    },
    #[error("the selected folder is not inside a Git repository")]
    NotRepository,
    #[error("'{0}' is not a changed file in this project")]
    NotChanged(String),
    #[error("invalid project-relative Git path '{0}'")]
    InvalidPath(String),
    #[error("'{0}' is not a file")]
    NotAFile(String),
    #[error("Git path '{0}' resolves outside the selected project")]
    OutsideProject(String),
    #[error("could not resolve Git path '{path}': {source}")]
    ResolvePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Git returned malformed porcelain status data")]
    MalformedStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "caelum-git-test-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn init_repository(&self) -> PathBuf {
            let project = self.0.join("project");
            fs::create_dir_all(&project).expect("create project directory");
            git(&project, &["init", "--quiet"]);
            git(&project, &["config", "user.name", "Caelum Tests"]);
            git(
                &project,
                &["config", "user.email", "caelum-tests@example.invalid"],
            );
            fs::write(project.join("tracked.txt"), "original\n").expect("create tracked file");
            git(&project, &["add", "tracked.txt"]);
            git(&project, &["commit", "--quiet", "-m", "initial"]);
            project
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git(directory: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(args)
            .output()
            .expect("run Git in test");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn returns_branch_and_changed_files_with_staged_unstaged_and_untracked_state() {
        let directory = TestDirectory::new("status");
        let project = directory.init_repository();
        fs::write(project.join("tracked.txt"), "staged change\n").unwrap();
        git(&project, &["add", "tracked.txt"]);
        fs::write(
            project.join("tracked.txt"),
            "staged change\nunstaged change\n",
        )
        .unwrap();
        fs::write(project.join("new.txt"), "new file\n").unwrap();
        fs::write(project.join("line\nbreak.txt"), "newline path\n").unwrap();

        let status = GitClient::new().repository_status(&project).unwrap();
        assert!(status
            .repository_root
            .as_deref()
            .is_some_and(|root| root == project.canonicalize().unwrap().to_string_lossy()));
        assert!(status.branch.is_some());
        assert_eq!(status.changes.len(), 3);
        let tracked = status
            .changes
            .iter()
            .find(|change| change.path == "tracked.txt")
            .unwrap();
        assert_eq!(tracked.index_status, "M");
        assert_eq!(tracked.worktree_status, "M");
        assert!(status
            .changes
            .iter()
            .any(|change| change.path == "line\nbreak.txt"));
        assert!(status.changes.iter().any(|change| change.path == "new.txt"));
    }

    #[test]
    fn returns_separate_staged_unstaged_and_untracked_diffs() {
        let directory = TestDirectory::new("diff");
        let project = directory.init_repository();
        fs::write(project.join("tracked.txt"), "staged change\n").unwrap();
        git(&project, &["add", "tracked.txt"]);
        fs::write(
            project.join("tracked.txt"),
            "staged change\nunstaged change\n",
        )
        .unwrap();
        fs::write(project.join("new.txt"), "new file body\n").unwrap();

        let tracked = GitClient::new().file_diff(&project, "tracked.txt").unwrap();
        assert!(tracked.staged.contains("staged change"));
        assert!(!tracked.staged.contains("unstaged change"));
        assert!(tracked.unstaged.contains("unstaged change"));
        assert!(tracked.untracked.is_empty());

        let untracked = GitClient::new().file_diff(&project, "new.txt").unwrap();
        assert!(untracked.staged.is_empty());
        assert!(untracked.unstaged.is_empty());
        assert!(untracked.untracked.contains("new file body"));
    }

    #[test]
    fn non_repository_projects_return_an_empty_repository_snapshot() {
        let directory = TestDirectory::new("not-repository");
        let project = directory.0.join("plain-folder");
        fs::create_dir_all(&project).unwrap();

        let status = GitClient::new().repository_status(&project).unwrap();
        assert_eq!(status.repository_root, None);
        assert_eq!(status.branch, None);
        assert!(status.changes.is_empty());
    }

    #[test]
    fn rejects_paths_outside_project_and_paths_that_are_not_changed() {
        let directory = TestDirectory::new("path-safety");
        let project = directory.init_repository();
        fs::write(directory.0.join("outside.txt"), "outside\n").unwrap();

        assert!(matches!(
            GitClient::new().file_diff(&project, "../outside.txt"),
            Err(GitError::InvalidPath(_))
        ));
        assert!(matches!(
            GitClient::new().file_diff(&project, "tracked.txt"),
            Err(GitError::NotChanged(_))
        ));
    }

    #[test]
    fn reports_git_command_failures() {
        let directory = TestDirectory::new("git-error");
        let project = directory.init_repository();
        let client = GitClient::with_executable(directory.0.join("missing-git"));

        assert!(matches!(
            client.repository_status(&project),
            Err(GitError::Unavailable { .. })
        ));
    }

    #[test]
    fn porcelain_parser_rejects_malformed_records() {
        assert!(matches!(
            parse_status(b"M bad\0"),
            Err(GitError::MalformedStatus)
        ));
        assert!(parse_status(b"").unwrap().is_empty());
    }

    #[test]
    fn detects_repository_root_when_opening_a_nested_project_folder() {
        let directory = TestDirectory::new("nested-project");
        let repository = directory.init_repository();
        let nested_project = repository.join("nested");
        fs::create_dir(&nested_project).unwrap();
        fs::write(nested_project.join("nested.txt"), "nested change\n").unwrap();
        fs::write(
            repository.join("outside-project.txt"),
            "outside selected folder\n",
        )
        .unwrap();

        let status = GitClient::new().repository_status(&nested_project).unwrap();
        assert_eq!(
            status.repository_root.as_deref(),
            Some(
                repository
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].path, "nested.txt");
        let diff = GitClient::new()
            .file_diff(&nested_project, "nested.txt")
            .unwrap();
        assert!(diff.untracked.contains("nested change"));
    }

    #[cfg(unix)]
    #[test]
    fn reports_git_command_failures_with_the_git_error_message() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new("command-error");
        let project = directory.init_repository();
        let executable = directory.0.join("failing-git");
        fs::write(
            &executable,
            "#!/bin/sh\necho 'intentional Git failure' >&2\nexit 42\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        assert!(matches!(
            GitClient::with_executable(executable).repository_status(&project),
            Err(GitError::CommandFailed { message, .. }) if message == "intentional Git failure"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_diff_symlinks_that_escape_the_selected_project() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("outside-symlink");
        let project = directory.init_repository();
        let outside = directory.0.join("outside.txt");
        fs::write(&outside, "outside\n").unwrap();
        symlink(&outside, project.join("linked.txt")).unwrap();
        git(&project, &["add", "linked.txt"]);

        assert!(matches!(
            GitClient::new().file_diff(&project, "linked.txt"),
            Err(GitError::Project(ProjectError::OutsideProject { .. }))
        ));
    }
}

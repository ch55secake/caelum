use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
    sync::{Arc, Mutex},
};

use caelum_protocol::{GitWorktree, GitWorktrees, ProjectInfo};
use thiserror::Error;

use crate::projects::{Project, ProjectError};

/// Git owns checkout discovery and removal. The application only chooses a
/// destination and remembers how checkouts are grouped in its workspace.
#[derive(Clone)]
pub struct WorktreeClient {
    home: PathBuf,
    operations: Arc<Mutex<()>>,
}

impl WorktreeClient {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            operations: Arc::new(Mutex::new(())),
        }
    }

    pub fn list(
        &self,
        path: impl AsRef<Path>,
        base_directory: &str,
    ) -> Result<Option<GitWorktrees>, WorktreeError> {
        let project = Project::open(path)?;
        let root = git(project.path(), &["rev-parse", "--show-toplevel"])?;
        if !root.status.success() {
            if String::from_utf8_lossy(&root.stderr).contains("not a git repository") {
                return Ok(None);
            }
            return Err(command_error(root));
        }
        let root = PathBuf::from(text(&root.stdout)).canonicalize()?;
        let prefix = project
            .path()
            .strip_prefix(&root)
            .map_err(|_| WorktreeError::InvalidPath)?;
        let common = checked(
            project.path(),
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let common = PathBuf::from(text(&common)).canonicalize()?;
        let records = checked(project.path(), &["worktree", "list", "--porcelain", "-z"])?;
        let mut worktrees = parse_worktrees(&records, prefix)?;
        let primary = worktrees.first().ok_or(WorktreeError::NotRepository)?;
        let group = primary.project.clone();
        let name = Path::new(&primary.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let storage_directory = self.base_directory(base_directory)?.join(format!(
            "{}-{:016x}",
            slug(&name),
            path_id(&common)
        ));
        let branches = checked(
            project.path(),
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads",
                "refs/remotes",
            ],
        )?;
        let branches: Vec<String> = text(&branches)
            .lines()
            .filter(|name| !name.ends_with("/HEAD"))
            .map(str::to_owned)
            .collect();
        let head = git(project.path(), &["rev-parse", "--verify", "HEAD"])?;
        let current_branch = worktrees
            .iter()
            .find(|entry| Path::new(&entry.path) == root)
            .and_then(|entry| entry.branch.clone());
        let default_base = ["main", "origin/main"]
            .into_iter()
            .find(|base| branches.iter().any(|branch| branch == base))
            .map(str::to_owned)
            .or(current_branch)
            .unwrap_or_else(|| {
                if head.status.success() {
                    text(&head.stdout)
                } else {
                    "HEAD".into()
                }
            });
        // Opening a repository subdirectory keeps that scope in every checkout.
        for entry in &mut worktrees {
            entry.project.name.clone_from(&group.name);
        }
        Ok(Some(GitWorktrees {
            project: group,
            storage_directory: storage_directory.to_string_lossy().into_owned(),
            branches,
            default_base,
            worktrees,
        }))
    }

    pub fn destination(
        &self,
        path: impl AsRef<Path>,
        branch: &str,
        base_directory: &str,
    ) -> Result<PathBuf, WorktreeError> {
        let project = Project::open(path)?;
        validate_branch(project.path(), branch)?;
        let listing = self
            .list(project.path(), base_directory)?
            .ok_or(WorktreeError::NotRepository)?;
        Ok(PathBuf::from(listing.storage_directory).join(slug(branch)))
    }

    pub fn create(
        &self,
        path: impl AsRef<Path>,
        branch: &str,
        base: &str,
        base_directory: &str,
    ) -> Result<GitWorktree, WorktreeError> {
        let _guard = self
            .operations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let project = Project::open(path)?;
        let destination = self.destination(project.path(), branch, base_directory)?;
        // Refuse existing directories and dangling symlinks before Git creates a branch.
        if destination.symlink_metadata().is_ok() {
            return Err(WorktreeError::Occupied(destination));
        }
        let revision = checked(
            project.path(),
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{base}^{{commit}}"),
            ],
        )?;
        let root = checked(project.path(), &["rev-parse", "--show-toplevel"])?;
        let root = PathBuf::from(text(&root)).canonicalize()?;
        let prefix = project
            .path()
            .strip_prefix(&root)
            .map_err(|_| WorktreeError::InvalidPath)?;
        if !prefix.as_os_str().is_empty() {
            let kind = checked(
                project.path(),
                &[
                    "cat-file",
                    "-t",
                    &format!("{}:{}", text(&revision), prefix.to_string_lossy()),
                ],
            )?;
            if text(&kind) != "tree" {
                return Err(WorktreeError::MissingProjectDirectory);
            }
        }
        let parent = destination.parent().ok_or(WorktreeError::InvalidPath)?;
        fs::create_dir_all(parent)?;
        let destination = parent
            .canonicalize()?
            .join(destination.file_name().ok_or(WorktreeError::InvalidPath)?);
        checked(
            project.path(),
            &[
                "worktree",
                "add",
                "-b",
                branch,
                "--",
                &destination.to_string_lossy(),
                &text(&revision),
            ],
        )?;
        self.list(project.path(), base_directory)?
            .and_then(|listing| {
                listing
                    .worktrees
                    .into_iter()
                    .find(|entry| Path::new(&entry.path) == destination)
            })
            .ok_or(WorktreeError::UnknownWorktree)
    }

    pub fn remove(
        &self,
        project_path: impl AsRef<Path>,
        worktree_path: &str,
    ) -> Result<GitWorktree, WorktreeError> {
        let _guard = self
            .operations
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let project = Project::open(project_path)?;
        let entry = self
            .list(project.path(), "")?
            .ok_or(WorktreeError::NotRepository)?
            .worktrees
            .into_iter()
            .find(|entry| entry.path == worktree_path)
            .ok_or(WorktreeError::UnknownWorktree)?;
        if entry.main {
            return Err(WorktreeError::MainWorktree);
        }
        if entry.locked.is_some() {
            return Err(WorktreeError::Locked);
        }
        // No force flag: Git refuses tracked changes, untracked files and
        // submodules. Removing a checkout leaves its branch and commits intact.
        checked(project.path(), &["worktree", "remove", "--", &entry.path])?;
        Ok(entry)
    }

    fn base_directory(&self, value: &str) -> Result<PathBuf, WorktreeError> {
        let value = value.trim();
        let path = if value.is_empty() {
            self.home.join(".caelum/worktrees")
        } else if value == "~" {
            self.home.clone()
        } else if let Some(relative) = value.strip_prefix("~/") {
            self.home.join(relative)
        } else {
            PathBuf::from(value)
        };
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(WorktreeError::InvalidPath);
        }
        Ok(path)
    }
}

fn parse_worktrees(bytes: &[u8], prefix: &Path) -> Result<Vec<GitWorktree>, WorktreeError> {
    let mut entries = Vec::new();
    let mut current: Option<GitWorktree> = None;
    for field in bytes.split(|byte| *byte == 0) {
        let field = std::str::from_utf8(field).map_err(|_| WorktreeError::InvalidPath)?;
        if let Some(path) = field.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            let directory = Path::new(path).join(prefix);
            let resolved = directory.canonicalize().ok().filter(|directory| {
                directory.is_dir()
                    && Path::new(path)
                        .canonicalize()
                        .is_ok_and(|root| directory.starts_with(root))
            });
            let available = resolved.is_some();
            let directory = resolved.unwrap_or(directory);
            current = Some(GitWorktree {
                path: path.to_owned(),
                project: ProjectInfo {
                    name: directory
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path: directory.to_string_lossy().into_owned(),
                },
                branch: None,
                main: entries.is_empty(),
                locked: None,
                prunable: None,
                available,
            });
        } else if let Some(entry) = current.as_mut() {
            if let Some(branch) = field.strip_prefix("branch refs/heads/") {
                entry.branch = Some(branch.to_owned());
            }
            if field == "locked" || field.starts_with("locked ") {
                entry.locked = Some(field.strip_prefix("locked ").unwrap_or("").to_owned());
            }
            if field == "prunable" || field.starts_with("prunable ") {
                entry.prunable = Some(field.strip_prefix("prunable ").unwrap_or("").to_owned());
            }
            if field == "bare" {
                return Err(WorktreeError::NotRepository);
            }
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    Ok(entries)
}

fn validate_branch(path: &Path, branch: &str) -> Result<(), WorktreeError> {
    if branch.is_empty() || branch.len() > 200 || branch.starts_with('-') {
        return Err(WorktreeError::InvalidBranch);
    }
    let result = git(path, &["check-ref-format", &format!("refs/heads/{branch}")])?;
    if !result.status.success() {
        return Err(WorktreeError::InvalidBranch);
    }
    Ok(())
}

fn slug(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(100)
        .collect();
    if result.is_empty() {
        "project".into()
    } else {
        result
    }
}

// Stable across app versions and processes, unlike DefaultHasher. Use the
// common Git directory so linked checkouts share an ID but separate clones do not.
fn path_id(path: &Path) -> u64 {
    path.as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn git(path: &Path, args: &[&str]) -> Result<Output, WorktreeError> {
    Ok(Command::new("git")
        .current_dir(path)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env("LC_ALL", "C")
        .output()?)
}

fn checked(path: &Path, args: &[&str]) -> Result<Vec<u8>, WorktreeError> {
    let output = git(path, args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(command_error(output))
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.strip_suffix(b"\n").unwrap_or(bytes)).into_owned()
}
fn command_error(output: Output) -> WorktreeError {
    WorktreeError::Git(String::from_utf8_lossy(&output.stderr).trim().to_owned())
}

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("worktree filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git: {0}")]
    Git(String),
    #[error("Open a Git checkout to manage worktrees.")]
    NotRepository,
    #[error("Choose an absolute worktree folder without '..' components.")]
    InvalidPath,
    #[error("Enter a valid new Git branch name.")]
    InvalidBranch,
    #[error("The worktree destination already exists: {0}")]
    Occupied(PathBuf),
    #[error("This worktree no longer belongs to the project. Refresh the worktree list.")]
    UnknownWorktree,
    #[error("The original checkout cannot be removed.")]
    MainWorktree,
    #[error("This worktree is locked. Unlock it in Git before removing it.")]
    Locked,
    #[error("The selected base does not contain this project's directory.")]
    MissingProjectDirectory,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Repository {
        directory: PathBuf,
        repo: PathBuf,
        client: WorktreeClient,
    }
    impl Repository {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "caelum-worktrees-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let repo = directory.join("project with spaces");
            fs::create_dir_all(repo.join("src")).unwrap();
            checked(&repo, &["init", "-b", "main"]).unwrap();
            fs::write(repo.join("src/file.txt"), "original\n").unwrap();
            checked(&repo, &["add", "."]).unwrap();
            checked(
                &repo,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    "initial",
                ],
            )
            .unwrap();
            let client = WorktreeClient::new(directory.join("home"));
            Self {
                directory,
                repo: repo.canonicalize().unwrap(),
                client,
            }
        }
    }
    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn creates_discovers_and_removes_real_worktrees_without_deleting_branches() {
        let test = Repository::new();
        let before = test.client.list(&test.repo, "").unwrap().unwrap();
        assert_eq!(before.worktrees.len(), 1);
        assert_eq!(before.default_base, "main");
        let preview = test
            .client
            .destination(&test.repo, "feature/search", "")
            .unwrap();
        assert!(preview.starts_with(test.directory.join("home/.caelum/worktrees")));
        let entry = test
            .client
            .create(&test.repo, "feature/search", "main", "")
            .unwrap();
        assert_eq!(
            entry.path,
            preview.canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(
            fs::read_to_string(Path::new(&entry.path).join("src/file.txt")).unwrap(),
            "original\n"
        );
        let from_checkout = test.client.list(&entry.path, "").unwrap().unwrap();
        assert_eq!(from_checkout.default_base, "main");
        assert_eq!(from_checkout.project, before.project);
        assert_eq!(from_checkout.storage_directory, before.storage_directory);
        assert_eq!(from_checkout.worktrees.len(), 2);
        test.client.remove(&test.repo, &entry.path).unwrap();
        assert!(!Path::new(&entry.path).exists());
        checked(
            &test.repo,
            &["rev-parse", "--verify", "refs/heads/feature/search"],
        )
        .unwrap();
        assert_eq!(
            test.client
                .list(&test.repo, "")
                .unwrap()
                .unwrap()
                .worktrees
                .len(),
            1
        );
    }

    #[test]
    fn refuses_dirty_locked_original_and_foreign_checkouts() {
        let test = Repository::new();
        let other = Repository::new();
        let entry = test
            .client
            .create(&test.repo, "feature", "main", "")
            .unwrap();
        let path = Path::new(&entry.path);
        fs::write(path.join("src/file.txt"), "changed").unwrap();
        assert!(test.client.remove(&test.repo, &entry.path).is_err());
        fs::write(path.join("src/file.txt"), "original\n").unwrap();
        fs::write(path.join("unsaved.txt"), "keep").unwrap();
        assert!(test.client.remove(&test.repo, &entry.path).is_err());
        fs::remove_file(path.join("unsaved.txt")).unwrap();
        checked(
            &test.repo,
            &["worktree", "lock", "--reason", "in use", &entry.path],
        )
        .unwrap();
        assert!(matches!(
            test.client.remove(&test.repo, &entry.path),
            Err(WorktreeError::Locked)
        ));
        assert!(matches!(
            test.client.remove(&test.repo, &test.repo.to_string_lossy()),
            Err(WorktreeError::MainWorktree)
        ));
        assert!(matches!(
            other.client.remove(&other.repo, &entry.path),
            Err(WorktreeError::UnknownWorktree)
        ));
        assert!(path.is_dir());
    }

    #[test]
    fn validates_names_locations_and_bases_without_overwriting_files() {
        let test = Repository::new();
        for branch in ["", "-force", "../outside", "bad name", "@{-1}"] {
            assert!(test.client.create(&test.repo, branch, "main", "").is_err());
        }
        assert!(test
            .client
            .create(&test.repo, "bad-base", "--help", "")
            .is_err());
        assert!(test
            .client
            .destination(&test.repo, "feature", "relative/path")
            .is_err());
        assert!(test
            .client
            .destination(&test.repo, "feature", "~/../outside")
            .is_err());
        let custom = test.directory.join("custom location");
        let entry = test
            .client
            .create(&test.repo, "feature", "main", &custom.to_string_lossy())
            .unwrap();
        assert!(Path::new(&entry.path).starts_with(custom.canonicalize().unwrap()));
        assert!(test
            .client
            .create(&test.repo, "feature", "main", "")
            .is_err());
        let occupied = test.client.destination(&test.repo, "occupied", "").unwrap();
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("keep"), "existing").unwrap();
        assert!(matches!(
            test.client.create(&test.repo, "occupied", "main", ""),
            Err(WorktreeError::Occupied(_))
        ));
        assert_eq!(
            fs::read_to_string(occupied.join("keep")).unwrap(),
            "existing"
        );
        assert!(
            !git(&test.repo, &["show-ref", "--verify", "refs/heads/occupied"])
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn separate_clones_have_different_storage_and_subprojects_keep_their_scope() {
        let first = Repository::new();
        let second = Repository::new();
        let a = first.client.list(&first.repo, "").unwrap().unwrap();
        let b = first.client.list(&second.repo, "").unwrap().unwrap();
        assert_ne!(a.storage_directory, b.storage_directory);
        let child = first
            .client
            .create(first.repo.join("src"), "nested", "main", "")
            .unwrap();
        assert_eq!(
            Path::new(&child.project.path),
            Path::new(&child.path).join("src")
        );
        let nested = first.client.list(&child.project.path, "").unwrap().unwrap();
        assert_eq!(Path::new(&nested.project.path), first.repo.join("src"));
        assert_eq!(nested.storage_directory, a.storage_directory);
    }

    #[test]
    fn discovers_external_detached_missing_and_newline_paths() {
        let test = Repository::new();
        let path = test.directory.join("external\ncheckout");
        checked(
            &test.repo,
            &[
                "worktree",
                "add",
                "--detach",
                &path.to_string_lossy(),
                "HEAD",
            ],
        )
        .unwrap();
        let listing = test.client.list(&test.repo, "").unwrap().unwrap();
        let external = listing.worktrees.iter().find(|entry| !entry.main).unwrap();
        assert_eq!(external.branch, None);
        assert_eq!(Path::new(&external.path), path.canonicalize().unwrap());
        fs::remove_dir_all(&path).unwrap();
        let listing = test.client.list(&test.repo, "").unwrap().unwrap();
        let external = listing.worktrees.iter().find(|entry| !entry.main).unwrap();
        assert!(!external.available);
        assert!(external.prunable.is_some());
        test.client.remove(&test.repo, &external.path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scoped_worktrees_do_not_follow_directories_outside_the_checkout() {
        let test = Repository::new();
        let entry = test
            .client
            .create(&test.repo, "linked", "main", "")
            .unwrap();
        let scoped = Path::new(&entry.path).join("src");
        fs::remove_dir_all(&scoped).unwrap();
        std::os::unix::fs::symlink(test.repo.join("src"), &scoped).unwrap();
        let listing = test
            .client
            .list(test.repo.join("src"), "")
            .unwrap()
            .unwrap();
        let linked = listing.worktrees.iter().find(|entry| !entry.main).unwrap();
        assert!(!linked.available);
        assert_eq!(Path::new(&linked.project.path), scoped);
        assert_ne!(linked.project.path, listing.project.path);

        fs::create_dir(test.repo.join("uncommitted")).unwrap();
        assert!(test
            .client
            .create(test.repo.join("uncommitted"), "missing-scope", "main", "")
            .is_err());
        assert!(!git(
            &test.repo,
            &["show-ref", "--verify", "refs/heads/missing-scope"]
        )
        .unwrap()
        .status
        .success());
    }

    #[test]
    fn lists_unborn_repositories_and_leaves_non_git_folders_alone() {
        let test = Repository::new();
        let empty = test.directory.join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(test.client.list(&empty, "").unwrap().is_none());
        checked(&empty, &["init", "-b", "main"]).unwrap();
        let listing = test.client.list(&empty, "").unwrap().unwrap();
        assert_eq!(listing.worktrees.len(), 1);
        assert!(listing.worktrees[0].available);
        assert_eq!(listing.default_base, "main");
        assert!(test.client.create(&empty, "first", "main", "").is_err());
    }
}

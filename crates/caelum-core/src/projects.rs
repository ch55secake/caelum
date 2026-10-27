use std::path::{Path, PathBuf};

use caelum_protocol::ProjectInfo;
use thiserror::Error;

/// A local project directory opened by Caelum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    path: PathBuf,
    name: String,
}

impl Project {
    /// Resolve a path and ensure it points to an existing directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(ProjectError::EmptyPath);
        }
        let canonical_path = path
            .canonicalize()
            .map_err(|source| ProjectError::Resolve {
                path: path.to_path_buf(),
                source,
            })?;

        if !canonical_path.is_dir() {
            return Err(ProjectError::NotDirectory(canonical_path));
        }

        let name = canonical_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| canonical_path.display().to_string());

        Ok(Self {
            path: canonical_path,
            name,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolve an existing filesystem path and ensure it remains inside this
    /// canonical project root. Canonicalization also rejects symlinks that
    /// point outside the selected project.
    pub fn resolve_scoped_path(&self, path: impl AsRef<Path>) -> Result<PathBuf, ProjectError> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(ProjectError::EmptyPath);
        }

        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path.join(path)
        };
        let canonical_path = candidate
            .canonicalize()
            .map_err(|source| ProjectError::Resolve {
                path: candidate.clone(),
                source,
            })?;

        if !canonical_path.starts_with(&self.path) {
            return Err(ProjectError::OutsideProject {
                path: canonical_path,
                project: self.path.clone(),
            });
        }

        Ok(canonical_path)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn info(&self) -> ProjectInfo {
        ProjectInfo {
            name: self.name.clone(),
            path: self.path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("project path cannot be empty")]
    EmptyPath,
    #[error("could not resolve project path '{path}': {source}")]
    Resolve {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("project path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("path '{path}' is outside the selected project '{project}'")]
    OutsideProject { path: PathBuf, project: PathBuf },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "caelum-project-test-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn opens_existing_directory_and_returns_canonical_path() {
        let project = Project::open(env!("CARGO_MANIFEST_DIR")).expect("crate directory exists");

        assert!(project.path().is_absolute());
        assert!(project.path().is_dir());
        assert!(!project.name().is_empty());
    }

    #[test]
    fn rejects_a_file_as_a_project() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert!(matches!(
            Project::open(file),
            Err(ProjectError::NotDirectory(_))
        ));
    }

    #[test]
    fn rejects_an_empty_project_path() {
        assert!(matches!(Project::open(""), Err(ProjectError::EmptyPath)));
    }

    #[test]
    fn resolves_paths_inside_the_project_root() {
        let directory = TestDirectory::new("inside");
        let project = Project::open(&directory.0).expect("open test project");
        let child = directory.0.join("src");
        std::fs::create_dir(&child).expect("create project child");

        assert_eq!(
            project.resolve_scoped_path("src").unwrap(),
            child.canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_paths_outside_the_project_root() {
        let parent = TestDirectory::new("outside");
        let project_path = parent.0.join("project");
        let outside_path = parent.0.join("outside-file");
        std::fs::create_dir(&project_path).expect("create project directory");
        std::fs::write(&outside_path, "outside").expect("create outside file");
        let project = Project::open(&project_path).expect("open test project");

        assert!(matches!(
            project.resolve_scoped_path(&outside_path),
            Err(ProjectError::OutsideProject { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_project_root() {
        use std::os::unix::fs::symlink;

        let parent = TestDirectory::new("symlink");
        let project_path = parent.0.join("project");
        let outside_path = parent.0.join("outside");
        std::fs::create_dir(&project_path).expect("create project directory");
        std::fs::create_dir(&outside_path).expect("create outside directory");
        symlink(&outside_path, project_path.join("linked"))
            .expect("create symlink outside project");
        let project = Project::open(&project_path).expect("open test project");

        assert!(matches!(
            project.resolve_scoped_path("linked"),
            Err(ProjectError::OutsideProject { .. })
        ));
    }
}

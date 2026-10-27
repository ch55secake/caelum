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
    #[error("could not resolve project path '{path}': {source}")]
    Resolve {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("project path is not a directory: {0}")]
    NotDirectory(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

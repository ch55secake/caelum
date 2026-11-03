use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use super::GitError;
use crate::projects::Project;

pub(super) fn project_relative_prefix(
    project: &Project,
    repository_root: &Path,
) -> Result<PathBuf, GitError> {
    project
        .path()
        .strip_prefix(repository_root)
        .map(Path::to_path_buf)
        .map_err(|_| GitError::OutsideProject(project.path().display().to_string()))
}

pub(super) fn git_path_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn validate_project_relative_path(path: &str) -> Result<PathBuf, GitError> {
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

pub(super) fn validate_scoped_file(
    project: &Project,
    relative_path: &Path,
) -> Result<(), GitError> {
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

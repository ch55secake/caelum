use std::{
    fs::{self, File},
    io::{Read, Take},
    path::{Component, Path, PathBuf},
};

use caelum_protocol::{
    ProjectDirectoryListing, ProjectEntryKind, ProjectFileContent, ProjectFileEntry,
};
use thiserror::Error;

use crate::projects::{Project, ProjectError};

pub const DEFAULT_DIRECTORY_PAGE_SIZE: usize = 200;
pub const MAX_DIRECTORY_PAGE_SIZE: usize = 500;
pub const MAX_TEXT_FILE_BYTES: usize = 1024 * 1024;

/// Read one directory level of an opened project. Child directories are loaded on demand.
pub fn list_directory(
    project_path: impl AsRef<Path>,
    relative_path: &str,
    offset: usize,
    limit: usize,
) -> Result<ProjectDirectoryListing, FileBrowserError> {
    let project = Project::open(project_path)?;
    let path = resolve_project_path(&project, relative_path, true)?;
    if !path.is_dir() {
        return Err(FileBrowserError::NotDirectory(relative_path.to_owned()));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&path).map_err(|source| FileBrowserError::ReadDirectory {
        path: path.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| FileBrowserError::ReadDirectoryEntry {
            path: path.clone(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }

        let file_type = entry
            .file_type()
            .map_err(|source| FileBrowserError::ReadEntry {
                path: entry.path(),
                source,
            })?;
        let (kind, size_bytes) = if file_type.is_dir() {
            (ProjectEntryKind::Directory, None)
        } else if file_type.is_symlink() {
            (ProjectEntryKind::Symlink, None)
        } else if file_type.is_file() {
            let metadata = entry
                .metadata()
                .map_err(|source| FileBrowserError::ReadEntry {
                    path: entry.path(),
                    source,
                })?;
            (ProjectEntryKind::File, Some(metadata.len()))
        } else {
            continue;
        };

        entries.push(ProjectFileEntry {
            path: join_relative_path(relative_path, &name),
            name,
            kind,
            size_bytes,
        });
    }

    entries.sort_by(|left, right| {
        entry_order(left.kind)
            .cmp(&entry_order(right.kind))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });

    let total = entries.len();
    let page_size = limit.clamp(1, MAX_DIRECTORY_PAGE_SIZE);
    let page = entries
        .into_iter()
        .skip(offset)
        .take(page_size)
        .collect::<Vec<_>>();
    let next_offset =
        (offset.saturating_add(page.len()) < total).then_some(offset.saturating_add(page.len()));

    Ok(ProjectDirectoryListing {
        path: relative_path.to_owned(),
        entries: page,
        total,
        next_offset,
    })
}

/// Read a bounded text preview for Monaco without loading an arbitrarily large file into memory.
pub fn read_text_file(
    project_path: impl AsRef<Path>,
    relative_path: &str,
) -> Result<ProjectFileContent, FileBrowserError> {
    let project = Project::open(project_path)?;
    let path = resolve_project_path(&project, relative_path, false)?;
    let metadata = fs::metadata(&path).map_err(|source| FileBrowserError::ReadFile {
        path: path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(FileBrowserError::NotFile(relative_path.to_owned()));
    }

    let file = File::open(&path).map_err(|source| FileBrowserError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::with_capacity(MAX_TEXT_FILE_BYTES + 1);
    let mut limited = file.take((MAX_TEXT_FILE_BYTES + 1) as u64);
    read_preview(&mut limited, &path, &mut bytes)?;

    let truncated =
        bytes.len() > MAX_TEXT_FILE_BYTES || metadata.len() > MAX_TEXT_FILE_BYTES as u64;
    if bytes.len() > MAX_TEXT_FILE_BYTES {
        bytes.truncate(MAX_TEXT_FILE_BYTES);
    }
    if bytes.contains(&0) {
        return Err(FileBrowserError::NotTextFile(relative_path.to_owned()));
    }

    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(error) if truncated && error.utf8_error().error_len().is_none() => {
            let valid_up_to = error.utf8_error().valid_up_to();
            let bytes = error.into_bytes();
            let valid_bytes = &bytes[..valid_up_to];
            String::from_utf8(valid_bytes.to_vec()).expect("valid UTF-8 prefix")
        }
        Err(_) => return Err(FileBrowserError::NotTextFile(relative_path.to_owned())),
    };

    Ok(ProjectFileContent {
        path: relative_path.to_owned(),
        content,
        size_bytes: metadata.len(),
        truncated,
    })
}

fn read_preview<R: Read>(
    reader: &mut Take<R>,
    path: &Path,
    bytes: &mut Vec<u8>,
) -> Result<(), FileBrowserError> {
    reader
        .read_to_end(bytes)
        .map(|_| ())
        .map_err(|source| FileBrowserError::ReadFile {
            path: path.to_path_buf(),
            source,
        })
}

fn resolve_project_path(
    project: &Project,
    relative_path: &str,
    allow_root: bool,
) -> Result<PathBuf, FileBrowserError> {
    if relative_path.is_empty() && allow_root {
        return Ok(project.path().to_path_buf());
    }
    validate_relative_path(relative_path)?;
    project
        .resolve_scoped_path(relative_path)
        .map_err(FileBrowserError::Project)
}

fn validate_relative_path(path: &str) -> Result<(), FileBrowserError> {
    let relative = Path::new(path);
    if path.is_empty()
        || path.contains('\0')
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FileBrowserError::InvalidPath(path.to_owned()));
    }
    Ok(())
}

fn join_relative_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    }
}

fn entry_order(kind: ProjectEntryKind) -> u8 {
    match kind {
        ProjectEntryKind::Directory => 0,
        ProjectEntryKind::File => 1,
        ProjectEntryKind::Symlink => 2,
    }
}

#[derive(Debug, Error)]
pub enum FileBrowserError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("invalid project-relative path '{0}'")]
    InvalidPath(String),
    #[error("'{0}' is not a directory")]
    NotDirectory(String),
    #[error("'{0}' is not a file")]
    NotFile(String),
    #[error("'{0}' is not a UTF-8 text file")]
    NotTextFile(String),
    #[error("could not read project directory '{path}': {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read an entry in project directory '{path}': {source}")]
    ReadDirectoryEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not inspect project entry '{path}': {source}")]
    ReadEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read project file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
                "caelum-files-test-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn project(&self) -> PathBuf {
            let path = self.0.join("project");
            fs::create_dir_all(&path).expect("create project directory");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_one_directory_level_with_directories_first_and_pagination() {
        let directory = TestDirectory::new("listing");
        let project = directory.project();
        fs::create_dir(project.join("z-folder")).unwrap();
        fs::create_dir(project.join("a-folder")).unwrap();
        fs::write(project.join("b.txt"), "b").unwrap();
        fs::write(project.join("a.txt"), "a").unwrap();
        fs::create_dir(project.join(".git")).unwrap();

        let first_page = list_directory(&project, "", 0, 2).unwrap();
        assert_eq!(first_page.total, 4);
        assert_eq!(first_page.entries[0].name, "a-folder");
        assert_eq!(first_page.entries[1].name, "z-folder");
        assert_eq!(first_page.next_offset, Some(2));

        let second_page = list_directory(&project, "", 2, 2).unwrap();
        assert_eq!(second_page.entries[0].name, "a.txt");
        assert_eq!(second_page.entries[1].name, "b.txt");
        assert_eq!(second_page.next_offset, None);
    }

    #[test]
    fn nested_directory_listing_and_file_read_are_project_scoped() {
        let directory = TestDirectory::new("nested");
        let project = directory.project();
        fs::create_dir(project.join("src")).unwrap();
        fs::write(project.join("src/main.rs"), "fn main() {}\n").unwrap();

        let listing = list_directory(&project, "src", 0, DEFAULT_DIRECTORY_PAGE_SIZE).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].path, "src/main.rs");
        assert_eq!(listing.entries[0].kind, ProjectEntryKind::File);

        let content = read_text_file(&project, "src/main.rs").unwrap();
        assert_eq!(content.content, "fn main() {}\n");
        assert!(!content.truncated);
    }

    #[test]
    fn large_text_files_return_a_bounded_preview() {
        let directory = TestDirectory::new("large-file");
        let project = directory.project();
        fs::write(
            project.join("large.txt"),
            vec![b'x'; MAX_TEXT_FILE_BYTES + 32],
        )
        .unwrap();

        let content = read_text_file(&project, "large.txt").unwrap();
        assert_eq!(content.content.len(), MAX_TEXT_FILE_BYTES);
        assert!(content.truncated);
        assert_eq!(content.size_bytes, (MAX_TEXT_FILE_BYTES + 32) as u64);
    }

    #[test]
    fn truncated_text_preview_ends_at_a_valid_utf8_boundary() {
        let directory = TestDirectory::new("utf8-boundary");
        let project = directory.project();
        let mut bytes = vec![b'a'; MAX_TEXT_FILE_BYTES - 1];
        bytes.extend_from_slice("€".as_bytes());
        fs::write(project.join("boundary.txt"), bytes).unwrap();

        let content = read_text_file(&project, "boundary.txt").unwrap();
        assert!(content.truncated);
        assert_eq!(content.content.len(), MAX_TEXT_FILE_BYTES - 1);
        assert!(content.content.is_char_boundary(content.content.len()));
    }

    #[test]
    fn rejects_binary_files_and_paths_outside_the_project() {
        let directory = TestDirectory::new("invalid-file");
        let project = directory.project();
        fs::write(project.join("binary.bin"), [0, 1, 2]).unwrap();
        fs::write(directory.0.join("outside.txt"), "outside").unwrap();

        assert!(matches!(
            read_text_file(&project, "binary.bin"),
            Err(FileBrowserError::NotTextFile(_))
        ));
        assert!(matches!(
            read_text_file(&project, "../outside.txt"),
            Err(FileBrowserError::InvalidPath(_))
        ));
        assert!(matches!(
            list_directory(&project, "missing", 0, 10),
            Err(FileBrowserError::Project(ProjectError::Resolve { .. }))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_project() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("symlink");
        let project = directory.project();
        let outside = directory.0.join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        symlink(&outside, project.join("outside-link.txt")).unwrap();

        assert!(matches!(
            read_text_file(&project, "outside-link.txt"),
            Err(FileBrowserError::Project(
                ProjectError::OutsideProject { .. }
            ))
        ));
    }
}

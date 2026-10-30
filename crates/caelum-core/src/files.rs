use std::{
    fs::{self, File},
    io::{Read, Take},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

use caelum_protocol::{
    ProjectDirectoryListing, ProjectEntryKind, ProjectFileContent, ProjectFileEntry,
    ProjectSearchMatch, ProjectSearchMode, ProjectSearchResults,
};
use thiserror::Error;

use crate::projects::{Project, ProjectError};

pub const DEFAULT_DIRECTORY_PAGE_SIZE: usize = 200;
pub const MAX_DIRECTORY_PAGE_SIZE: usize = 500;
pub const MAX_TEXT_FILE_BYTES: usize = 1024 * 1024;

/// Bounded, ignore-aware search. File search uses fuzzy path matching; content
/// search is a case-insensitive literal search with editor-ready coordinates.
pub fn search_project(
    project_path: impl AsRef<Path>,
    query: &str,
    mode: ProjectSearchMode,
    include_ignored: bool,
) -> Result<ProjectSearchResults, FileBrowserError> {
    const MAX_RESULTS: usize = 200;
    let project = Project::open(project_path)?;
    let query = match mode {
        ProjectSearchMode::Files => query.trim(),
        ProjectSearchMode::Content => query,
    };
    if query.chars().count() > 256 {
        return Err(FileBrowserError::QueryTooLong);
    }
    let mut result = ProjectSearchResults {
        matches: Vec::new(),
        truncated: false,
        skipped: 0,
    };
    if query.trim().is_empty() && mode == ProjectSearchMode::Content {
        return Ok(result);
    }
    let pattern = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
        .map_err(|_| FileBrowserError::QueryTooLong)?;
    let mut walker = ignore::WalkBuilder::new(project.path());
    walker
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .git_ignore(!include_ignored)
        .git_exclude(!include_ignored)
        .git_global(!include_ignored)
        .ignore(!include_ignored)
        .filter_entry(|entry| entry.file_name() != ".git");
    let started = Instant::now();
    let mut bytes_read = 0;
    let mut ranked = Vec::new();
    for (visited, entry) in walker.build().enumerate() {
        if visited >= 50_000
            || bytes_read >= 32 * MAX_TEXT_FILE_BYTES
            || started.elapsed() > Duration::from_secs(2)
        {
            result.truncated = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Some(path) = entry
            .path()
            .strip_prefix(project.path())
            .ok()
            .and_then(Path::to_str)
        else {
            result.skipped += 1;
            continue;
        };
        let path = path.replace(std::path::MAIN_SEPARATOR, "/");
        if mode == ProjectSearchMode::Files {
            if let Some(score) = path_score(&path, query) {
                ranked.push((score, path));
            }
            continue;
        }
        let file = match read_text_file(project.path(), &path) {
            Ok(file) => file,
            Err(FileBrowserError::NotTextFile(_)) => continue,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        bytes_read += file.content.len();
        result.truncated |= file.truncated;
        for (index, line) in file.content.lines().enumerate() {
            if let Some(found) = pattern.find(line) {
                if result.matches.len() == MAX_RESULTS {
                    result.truncated = true;
                    return Ok(result);
                }
                let before = &line[..found.start()];
                result.matches.push(ProjectSearchMatch {
                    path: path.clone(),
                    line: Some(index + 1),
                    column: Some(before.encode_utf16().count() + 1),
                    preview: Some(
                        line.chars()
                            .skip(before.chars().count().saturating_sub(60))
                            .take(240)
                            .collect(),
                    ),
                });
            }
        }
    }
    if mode == ProjectSearchMode::Files {
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        result.truncated |= ranked.len() > MAX_RESULTS;
        result.matches = ranked
            .into_iter()
            .take(MAX_RESULTS)
            .map(|(_, path)| ProjectSearchMatch {
                path,
                line: None,
                column: None,
                preview: None,
            })
            .collect();
    }
    Ok(result)
}

fn path_score(path: &str, query: &str) -> Option<i64> {
    let path = path.to_lowercase();
    let query = query.to_lowercase();
    let mut remaining = path.chars().enumerate();
    let mut gaps = 0;
    let mut previous = None;
    for character in query.chars() {
        let (index, _) = remaining.find(|(_, value)| *value == character)?;
        gaps += previous.map_or(index, |previous| index - previous - 1);
        previous = Some(index);
    }
    let name = path.rsplit('/').next().unwrap_or(&path);
    Some(
        if name == query {
            10_000
        } else if name.starts_with(&query) {
            5_000
        } else if name.contains(&query) {
            2_000
        } else {
            0
        } - gaps as i64
            - path.len() as i64,
    )
}

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

/// Read a bounded text preview without loading an arbitrarily large file into memory.
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
    #[error("Search queries must be at most 256 characters")]
    QueryTooLong,
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
    fn search_honors_ignore_files_and_excludes_git_metadata_even_when_including_ignored() {
        let directory = TestDirectory::new("search-ignore");
        let project = directory.project();
        fs::create_dir_all(project.join(".git/info")).unwrap();
        fs::create_dir(project.join("vendor")).unwrap();
        fs::write(project.join(".gitignore"), "vendor/\n").unwrap();
        fs::write(project.join(".ignore"), "local.txt\n").unwrap();
        fs::write(project.join(".git/info/exclude"), "excluded.txt\n").unwrap();
        for path in [
            "visible.txt",
            ".hidden.txt",
            "vendor/generated.txt",
            "local.txt",
            "excluded.txt",
            ".git/private.txt",
        ] {
            fs::write(project.join(path), "searchable marker").unwrap();
        }
        for mode in [ProjectSearchMode::Files, ProjectSearchMode::Content] {
            let query = if mode == ProjectSearchMode::Files {
                ".txt"
            } else {
                "marker"
            };
            let mut paths = search_project(&project, query, mode, false)
                .unwrap()
                .matches
                .into_iter()
                .map(|item| item.path)
                .collect::<Vec<_>>();
            paths.sort();
            assert_eq!(paths, [".hidden.txt", "visible.txt"]);
            let included = search_project(&project, query, mode, true).unwrap();
            assert_eq!(included.matches.len(), 5);
            assert!(included
                .matches
                .iter()
                .all(|item| !item.path.starts_with(".git/")));
        }
    }

    #[test]
    fn filename_search_is_fuzzy_case_insensitive_and_ranks_exact_names_first() {
        let directory = TestDirectory::new("search-fuzzy");
        let project = directory.project();
        fs::create_dir(project.join("src")).unwrap();
        for path in [
            "App.tsx",
            "App.test.tsx",
            "src/Application.tsx",
            "README.md",
        ] {
            fs::write(project.join(path), "").unwrap();
        }
        let matches = search_project(&project, "APP.TSX", ProjectSearchMode::Files, false)
            .unwrap()
            .matches;
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].path, "App.tsx");
        assert!(matches
            .iter()
            .all(|item| item.line.is_none() && item.preview.is_none()));
        let fuzzy = search_project(&project, "sApn", ProjectSearchMode::Files, false).unwrap();
        assert_eq!(fuzzy.matches[0].path, "src/Application.tsx");
    }

    #[test]
    fn content_search_is_literal_with_unicode_editor_coordinates_and_bounded_previews() {
        let directory = TestDirectory::new("search-content");
        let project = directory.project();
        fs::write(
            project.join("source.txt"),
            format!(
                "unmatched\r\n🚀éTeSt.*\r\n{}test.*{}\n",
                "é".repeat(300),
                "x".repeat(400)
            ),
        )
        .unwrap();
        fs::write(project.join("binary.bin"), b"test.*\0binary").unwrap();
        fs::write(project.join("invalid.bin"), [255, 254]).unwrap();
        let found = search_project(&project, "test.*", ProjectSearchMode::Content, false).unwrap();
        assert_eq!(found.matches.len(), 2);
        assert_eq!(
            (found.matches[0].line, found.matches[0].column),
            (Some(2), Some(4))
        );
        assert_eq!(found.matches[0].preview.as_deref(), Some("🚀éTeSt.*"));
        assert_eq!(found.matches[1].column, Some(301));
        assert_eq!(
            found.matches[1].preview.as_ref().unwrap().chars().count(),
            240
        );
        assert!(!found.truncated);
        assert_eq!(found.skipped, 0);
        assert!(
            search_project(&project, " test.*", ProjectSearchMode::Content, false)
                .unwrap()
                .matches
                .is_empty()
        );
        assert!(
            search_project(&project, "absent", ProjectSearchMode::Content, false)
                .unwrap()
                .matches
                .is_empty()
        );
    }

    #[test]
    fn search_bounds_results_queries_and_large_file_content() {
        let directory = TestDirectory::new("search-limits");
        let project = directory.project();
        for index in 0..205 {
            fs::write(project.join(format!("entry-{index}.txt")), "marker\n").unwrap();
        }
        for mode in [ProjectSearchMode::Files, ProjectSearchMode::Content] {
            let query = if mode == ProjectSearchMode::Files {
                "entry"
            } else {
                "marker"
            };
            let result = search_project(&project, query, mode, false).unwrap();
            assert_eq!(result.matches.len(), 200);
            assert!(result.truncated);
        }
        assert!(
            search_project(&project, "", ProjectSearchMode::Content, false)
                .unwrap()
                .matches
                .is_empty()
        );
        assert!(matches!(
            search_project(&project, &"x".repeat(257), ProjectSearchMode::Files, false),
            Err(FileBrowserError::QueryTooLong)
        ));
        fs::write(
            project.join("large.txt"),
            format!("{}tail-only", "x".repeat(MAX_TEXT_FILE_BYTES)),
        )
        .unwrap();
        let result =
            search_project(&project, "tail-only", ProjectSearchMode::Content, false).unwrap();
        assert!(result.matches.is_empty());
        assert!(result.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn search_stays_inside_the_project_without_following_file_or_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = TestDirectory::new("search-scope");
        let project = directory.project();
        let outside = directory.0.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("private.txt"), "marker").unwrap();
        fs::write(project.join("inside.txt"), "marker").unwrap();
        symlink(&outside, project.join("linked-directory")).unwrap();
        symlink(outside.join("private.txt"), project.join("linked-file.txt")).unwrap();
        for mode in [ProjectSearchMode::Files, ProjectSearchMode::Content] {
            let query = if mode == ProjectSearchMode::Files {
                ".txt"
            } else {
                "marker"
            };
            let result = search_project(&project, query, mode, true).unwrap();
            assert_eq!(result.matches.len(), 1);
            assert_eq!(result.matches[0].path, "inside.txt");
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

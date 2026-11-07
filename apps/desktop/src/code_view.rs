//! Shared virtualized source and patch surfaces for Files, Changes and reviews.
use rune_kit::diff::{self, DiffState, FileDiff};
use std::hash::{Hash, Hasher};

/// Routed only by the focused code surface, so two visible diffs cannot consume
/// each other's selection or steal copy/navigation from the composer.
pub fn key(state: &mut DiffState, ev: &rune::KeyEvent, tx: &mut rune::Tx) -> bool {
    match &ev.key {
        rune::Key::Text(value) if !ev.mods.cmd && !ev.mods.ctrl && !ev.mods.alt => {
            match value.as_str() {
                "j" => state.next_hunk(true, tx),
                "k" => state.next_hunk(false, tx),
                "]" => state.next_file(true, tx),
                "[" => state.next_file(false, tx),
                _ => false,
            }
        }
        rune::Key::Escape if state.selection().is_some() => {
            state.set_selection(None);
            true
        }
        _ => state.key(ev, tx),
    }
}

#[derive(Default)]
pub struct Document {
    pub diff: DiffState,
    pub position: Position,
    revision: u64,
}

#[derive(Default)]
pub struct Position(std::cell::Cell<Option<[f32; 2]>>);
impl Position {
    pub fn get(&self) -> Option<[f32; 2]> {
        self.0.get()
    }
    pub fn capture(&self, key: &str) {
        if let (Some(view), Some(rows), Some(view_origin), Some(rows_origin)) = (
            rune::memo::window_layout(&format!("{key}/scroll")),
            rune::memo::window_layout(&format!("{key}/scroll/rows")),
            rune::memo::layout(&format!("{key}/scroll")),
            rune::memo::layout(&format!("{key}/scroll/rows")),
        ) {
            // A virtual list's mounted rows start after the culled prefix.
            // Compare scrolled/unscrolled origins, not just the visible gap.
            self.0.set(Some([
                (rows_origin[0] - rows[0] - (view_origin[0] - view[0])).max(0.0),
                (rows_origin[1] - rows[1] - (view_origin[1] - view[1])).max(0.0),
            ]));
        }
    }
}

pub fn source_key(project: &str, path: &str) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (project, path).hash(&mut hash);
    format!("source-{:x}", hash.finish())
}

impl Document {
    pub fn patch(&mut self, path: &str, patch: &str) -> bool {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (path, patch).hash(&mut hash);
        let revision = hash.finish();
        if self.revision == revision {
            return false;
        }
        replace_files(&mut self.diff, tool_files(path, patch));
        self.revision = revision;
        true
    }
    pub fn sync(&mut self, path: &str, content: &str) -> bool {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (path, content).hash(&mut h);
        let revision = h.finish();
        if revision == self.revision {
            return false;
        }
        // A source document is a single unchanged range. This gives both modes
        // the same syntax cache, virtualization, line selection and navigation.
        let mut patch = format!(
            "--- {}\n+++ {}\n@@ -1,{n} +1,{n} @@\n",
            quoted("a", path),
            quoted("b", path),
            n = content.lines().count()
        );
        for line in content.lines() {
            patch.push(' ');
            patch.push_str(line);
            patch.push('\n');
        }
        if !content.is_empty() && !content.ends_with('\n') {
            patch.push_str("\\ No newline at end of file\n");
        }
        replace_files(&mut self.diff, diff::parse(&patch));
        self.revision = revision;
        true
    }
}

/// Tools also emit apply_patch syntax and old/new fragments without a unified
/// header. Normalize those once before handing them to the shared code surface.
pub fn tool_files(path: &str, patch: &str) -> Vec<FileDiff> {
    let parsed = diff::parse(patch);
    if !parsed.is_empty() {
        return parsed;
    }
    if patch.lines().any(|line| line.starts_with("@@ -")) && !patch.contains("*** Begin Patch") {
        return diff::parse(&format!(
            "--- {}\n+++ {}\n{patch}\n",
            quoted("a", path),
            quoted("b", path)
        ));
    }
    let mut files = Vec::new();
    let mut path = path.to_owned();
    let mut lines = Vec::new();
    let flush = |path: &str, lines: &mut Vec<String>, files: &mut Vec<FileDiff>| {
        if lines.is_empty() {
            return;
        }
        let old = lines.iter().filter(|l| !l.starts_with('+')).count();
        let new = lines.iter().filter(|l| !l.starts_with('-')).count();
        files.extend(diff::parse(&format!(
            "--- {}\n+++ {}\n@@ -1,{old} +1,{new} @@\n{}\n",
            quoted("a", path),
            quoted("b", path),
            lines.join("\n")
        )));
        lines.clear();
    };
    for line in patch.lines() {
        if let Some(name) = line
            .strip_prefix("*** Update File: ")
            .or_else(|| line.strip_prefix("*** Add File: "))
            .or_else(|| line.strip_prefix("*** Delete File: "))
        {
            flush(&path, &mut lines, &mut files);
            path = name.into();
        } else if !line.starts_with("***")
            && !line.starts_with("@@")
            && matches!(line.as_bytes().first(), Some(b'+' | b'-' | b' '))
        {
            lines.push(line.into());
        }
    }
    flush(&path, &mut lines, &mut files);
    files
}

fn quoted(prefix: &str, path: &str) -> String {
    serde_json::to_string(&format!("{prefix}/{path}")).expect("string")
}

pub fn replace_files(state: &mut DiffState, files: Vec<FileDiff>) {
    let collapsed: Vec<_> = state
        .files()
        .iter()
        .enumerate()
        .filter(|(i, _)| state.collapsed(*i))
        .map(|(_, f)| f.path().to_owned())
        .collect();
    let selection = state
        .selection()
        .and_then(|s| state.files().get(s.file).map(|f| (f.path().to_owned(), s)));
    state.set_files(files);
    for path in collapsed {
        if let Some(i) = state.file_index(&path) {
            state.set_collapsed(i, true);
        }
    }
    if let Some((path, mut selection)) = selection
        && let Some(i) = state.file_index(&path)
    {
        selection.file = i;
        state.set_selection(Some(selection));
    }
}

pub fn review_files(files: &[caelum_protocol::GitHubPullRequestFile]) -> Vec<FileDiff> {
    files
        .iter()
        .flat_map(|file| {
            let old = file.previous_path.as_deref().unwrap_or(&file.path);
            let old = if file.status == "added" {
                "/dev/null".into()
            } else {
                quoted("a", old)
            };
            let new = if file.status == "removed" {
                "/dev/null".into()
            } else {
                quoted("b", &file.path)
            };
            let patch = file.patch.as_deref().unwrap_or("");
            let mut parsed = diff::parse(&format!(
                "diff --git {} {}\n--- {old}\n+++ {new}\n{patch}\n",
                quoted("a", file.previous_path.as_deref().unwrap_or(&file.path)),
                quoted("b", &file.path)
            ));
            for diff in &mut parsed {
                diff.status = match file.status.as_str() {
                    "added" => diff::FileStatus::Added,
                    "removed" => diff::FileStatus::Deleted,
                    "renamed" => diff::FileStatus::Renamed,
                    "copied" => diff::FileStatus::Copied,
                    _ => diff::FileStatus::Modified,
                };
                // Rune's no-content surface is binary-only. The file header
                // supplies GitHub's more precise binary-or-omitted explanation.
                diff.binary |= file.patch.is_none();
            }
            parsed
        })
        .collect()
}

pub fn has_line(file: &FileDiff, side: diff::Side, line: u64) -> bool {
    file.hunks.iter().any(|h| {
        h.lines
            .iter()
            .any(|row| row.number(side).is_some_and(|n| u64::from(n) == line))
    })
}

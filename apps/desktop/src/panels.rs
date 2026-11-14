//! Native workspace views; all reads, mutations and response ownership are in core.
mod appearance;
mod changes;
mod feedback;
mod files;
mod launcher;
mod overlays;
mod queue;
mod review;

pub(crate) use appearance::appearance_settings;
use changes::changes;
pub use changes::select_change;
use files::files;
use files::find_next;
pub use launcher::open_launcher;
pub use overlays::overlays;
pub use queue::composer_context;
pub use queue::queue_summary;
pub use review::open_review;
use review::review;

use crate::ui::Button;
use crate::{
    Workspace,
    live::{live, live_mut},
    quiet_button, ui,
};
use caelum_core::application::{Task, Workspace as Application};
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text};
use rune_kit::{
    ButtonVariant, Kit,
    diff::{self, DiffState},
    glyphs,
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    Home,
    #[default]
    Conversation,
    Files,
    Changes,
    Worktrees,
    CreateWorktree,
    Shells,
    Review,
    Settings,
    Session,
    Sessions,
    Timeline,
    Usage,
    Stats,
    Image,
    Attachment,
    Launcher,
}

type Target = (String, String);
type Revision = (u64, Option<Target>, Option<Target>, Option<Target>);
type CachedReview = (DiffState, u64, u64, Option<String>);

pub struct Panels {
    pub settings_page: crate::settings::Page,
    pub settings_provider: caelum_core::application::Provider,
    pub settings_menu: Option<&'static str>,
    pub screen: Screen,
    pub inspector: Screen,
    pub overlay: Option<Screen>,
    pub searching_sessions: bool,
    pub sessions: TextEdit,
    pub archived: bool,
    pub launch_provider: caelum_core::application::Provider,
    documents: std::collections::BTreeMap<(String, String), crate::code_view::Document>,
    pub review_tab: usize,
    review_nav: Option<bool>,
    review_search: TextEdit,
    pub(crate) merge_method: usize,
    pub(crate) merge_menu: bool,
    pub(crate) expanded_checks: HashSet<Target>,
    expanded_threads: HashSet<String>,
    search: TextEdit,
    find: TextEdit,
    finding: bool,
    find_index: usize,
    content_search: bool,
    pub review: Option<(String, String)>,
    review_file: Option<String>,
    change_file: Option<(String, String)>,
    diff: DiffState,
    diff_hash: u64,
    annotations_hash: u64,
    synced: Option<Revision>,
    review_cache: std::collections::HashMap<Target, CachedReview>,
    pub pending_line: Option<(String, String, usize)>,
    active_document: Option<(String, String)>,
    pending_scroll: Option<(String, [f32; 2])>,
    review_positions: std::collections::HashMap<(String, String), crate::code_view::Position>,
    active_review: Option<(String, String)>,
    review_restore: Option<(Target, [f32; 2])>,
    change_diff: DiffState,
    change_hash: u64,
    change_positions: std::collections::HashMap<Target, crate::code_view::Position>,
    selection: Option<Selection>,
    preview_page: usize,
    pub worktree: Option<crate::worktree::Form>,
    pub rename_worktree: Option<crate::worktree::RenameForm>,
    pub(crate) directory: TextEdit,
    directory_loaded: bool,
}

struct Selection {
    project: String,
    path: String,
    text: String,
    lines: Option<(usize, usize)>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            settings_page: Default::default(),
            settings_provider: Default::default(),
            settings_menu: None,
            screen: Screen::Conversation,
            inspector: Screen::Files,
            overlay: None,
            searching_sessions: false,
            sessions: TextEdit::new(""),
            archived: false,
            launch_provider: Default::default(),
            documents: Default::default(),
            review_tab: 0,
            review_nav: None,
            review_search: TextEdit::new(""),
            merge_method: 0,
            merge_menu: false,
            expanded_checks: Default::default(),
            expanded_threads: Default::default(),
            search: TextEdit::new(""),
            find: TextEdit::new(""),
            finding: false,
            find_index: 0,
            content_search: false,
            review: None,
            review_file: None,
            change_file: None,
            diff: DiffState::new(diff::parse("")),
            diff_hash: 0,
            annotations_hash: 0,
            synced: None,
            review_cache: Default::default(),
            pending_line: None,
            active_document: None,
            pending_scroll: None,
            review_positions: Default::default(),
            active_review: None,
            review_restore: None,
            change_diff: DiffState::new(diff::parse("")),
            change_hash: 0,
            change_positions: Default::default(),
            selection: None,
            preview_page: 0,
            worktree: None,
            rename_worktree: None,
            directory: TextEdit::new(""),
            directory_loaded: false,
        }
    }
}

impl Panels {
    pub fn remember(&self, app: &mut Application) {
        for ((project, path), document) in &self.documents {
            if let Some(offset) = document.position.get() {
                let saved = app.preferences.files.entry(project.clone()).or_default();
                saved.scroll.insert(path.clone(), offset);
                saved
                    .line
                    .insert(path.clone(), (offset[1] / 22.0) as usize + 1);
            }
        }
    }
    pub fn reveal_line(&mut self, tx: &mut Tx) {
        if let Some((key, offset)) = self.pending_scroll.take() {
            // Restoration must be held through the first virtual-row measurements
            // after a surface remount, rather than gliding from an empty viewport.
            tx.scroll_to_instant(key, offset[0], offset[1]);
        }
        if let Some((project, path, line)) = &self.pending_line
            && self.active_document.as_ref() == Some(&(project.clone(), path.clone()))
            && let Some(document) = self.documents.get_mut(&(project.clone(), path.clone()))
        {
            document.diff.scroll_to_line(
                0,
                diff::LineRef::new(diff::Side::New, (*line).max(1) as u32),
                tx,
            );
            self.pending_line = None;
        }
    }
    pub fn sync(&mut self, app: &Application) -> bool {
        let form_changed = self.worktree.as_mut().is_some_and(|form| form.sync(app));
        if self.change_file.as_ref().is_some_and(|(p, f)| {
            app.services
                .checkouts
                .get(p)
                .and_then(|c| c.git.as_ref())
                .is_some_and(|g| !g.changes.iter().any(|c| &c.path == f))
        }) {
            self.change_file = None;
            self.change_diff = DiffState::default();
            self.change_hash = 0;
        }
        let document = app.active_project.as_ref().and_then(|p| {
            app.services
                .checkouts
                .get(p)?
                .selected_file
                .as_ref()
                .map(|f| (p.clone(), f.clone()))
        });
        if document != self.active_document {
            self.active_document = document.clone();
            if let Some((p, f)) = &document {
                let saved = app.preferences.files.get(p);
                if self.pending_line.is_none() {
                    if let Some(offset) = saved
                        .and_then(|s| s.scroll.get(f))
                        .copied()
                        .filter(|offset| offset.iter().all(|n| n.is_finite() && *n >= 0.0))
                    {
                        self.pending_scroll = Some((
                            format!("{}/scroll", crate::code_view::source_key(p, f)),
                            offset,
                        ));
                    } else if let Some(line) = saved.and_then(|s| s.line.get(f)) {
                        self.pending_line = Some((p.clone(), f.clone(), *line));
                    }
                }
            }
        }
        let review = (self.screen == Screen::Review && self.review_tab == 1)
            .then(|| self.review.clone())
            .flatten();
        if review != self.active_review {
            self.active_review = review.clone();
            if let Some(key) = review {
                let pos = self.review_positions.entry(key).or_default();
                if let Some(offset) = pos.get() {
                    self.pending_scroll = Some(("diff/scroll".into(), offset));
                }
            }
        }
        let signature = (
            app.services.revision,
            document,
            self.change_file.clone(),
            (self.screen == Screen::Review)
                .then(|| self.review.clone())
                .flatten(),
        );
        if self.synced.as_ref() == Some(&signature) {
            return form_changed;
        }
        self.synced = Some(signature);
        let mut changed = form_changed;
        if let Some((project, checkout)) = app
            .active_project
            .as_ref()
            .and_then(|p| app.services.checkouts.get(p).map(|c| (p, c)))
            && let Some(path) = &checkout.selected_file
            && let Some(file) = checkout.files.get(path)
        {
            changed |= self
                .documents
                .entry((project.clone(), path.clone()))
                .or_default()
                .sync(path, &file.content);
        }
        if let Some(patch) = self
            .change_file
            .as_ref()
            .and_then(|(p, f)| app.services.checkouts.get(p)?.diffs.get(f))
            .map(|d| format!("{}{}{}", d.staged, d.unstaged, d.untracked))
        {
            changed |= update_diff(&mut self.change_diff, &mut self.change_hash, &patch);
        }
        if self.screen == Screen::Review
            && let Some((key, review)) = self
                .review
                .as_ref()
                .and_then(|key| app.services.reviews.get(key).map(|r| (key, r)))
        {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut hash);
            review.head().hash(&mut hash);
            for file in &review.files {
                (&file.path, &file.patch, &file.previous_path, &file.status).hash(&mut hash);
            }
            let revision = hash.finish();
            if !review.files_loaded
                && !self.diff.files().is_empty()
                && self.review_restore.is_none()
            {
                self.review_restore = self
                    .review_positions
                    .get(key)
                    .and_then(|p| p.get())
                    .map(|position| (key.clone(), position));
            }
            if review.files_loaded && self.diff_hash != revision {
                crate::code_view::replace_files(
                    &mut self.diff,
                    crate::code_view::review_files(&review.files),
                );
                self.diff_hash = revision;
                self.annotations_hash = 0;
                if self.review_file.is_none() {
                    self.review_file = review.files.first().map(|f| f.path.clone());
                }
                changed = true;
            }
            if review.files_loaded
                && self
                    .review_restore
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == key)
                && (review.next_page.is_none()
                    || self
                        .review_file
                        .as_ref()
                        .is_none_or(|path| review.files.iter().any(|f| &f.path == path)))
            {
                let (_, offset) = self.review_restore.take().unwrap();
                self.pending_scroll = Some(("diff/scroll".into(), offset));
            }
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            for thread in &review.threads {
                (
                    &thread.id,
                    thread.line,
                    &thread.diff_side,
                    thread.is_resolved,
                    thread.is_outdated,
                    thread.comments.len(),
                )
                    .hash(&mut hash);
            }
            let revision = hash.finish();
            if self.annotations_hash != revision {
                let notes = review
                    .threads
                    .iter()
                    .filter(|t| !t.is_outdated)
                    .filter_map(|t| {
                        Some(diff::Annotation::new(
                            self.diff.file_index(&t.path)?,
                            diff::LineRef::new(
                                if t.diff_side == "LEFT" {
                                    diff::Side::Old
                                } else {
                                    diff::Side::New
                                },
                                t.line?.try_into().ok()?,
                            ),
                            &t.id,
                        ))
                    })
                    .collect();
                self.diff.set_annotations(notes);
                self.annotations_hash = revision;
                changed = true;
            }
        }
        changed
    }

    pub fn restoring_review(&self) -> Option<&Target> {
        let (key, _) = self.review_restore.as_ref()?;
        (self.screen == Screen::Review && self.review.as_ref() == Some(key)).then_some(key)
    }
}

fn update_diff(diff: &mut DiffState, revision: &mut u64, patch: &str) -> bool {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    patch.hash(&mut hash);
    let hash = hash.finish();
    if *revision != hash {
        crate::code_view::replace_files(diff, diff::parse(patch));
        *revision = hash;
        return true;
    }
    false
}

fn label(value: impl Into<String>, k: &Kit) -> El {
    text(value.into()).font(k.ty.ui).color(k.c.text_base).wrap()
}
fn action(label: &str, task: Task, s: &Workspace, k: &Kit) -> El {
    Button::new(label)
        .variant(ButtonVariant::Outline)
        .disabled(live(s).app.services.busy(&task))
        .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
        .build(k)
}
fn scroll(content: El) -> El {
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(content)
}

pub fn show(s: &mut Workspace, screen: Screen) {
    if screen == Screen::Worktrees
        && let Some(project) = live(s).app.active_project.clone()
    {
        crate::checkouts::manage(s, project);
        return;
    }
    if screen == Screen::Session {
        live_mut(s).session_menu = live(s).app.selected;
        return;
    }
    if matches!(screen, Screen::Files | Screen::Changes) {
        s.inspector.set_open(true);
    }
    let live = live_mut(s);
    match screen {
        Screen::Files | Screen::Changes => live.panels.inspector = screen,
        Screen::Settings
        | Screen::Worktrees
        | Screen::Session
        | Screen::Sessions
        | Screen::Timeline
        | Screen::Usage
        | Screen::Stats => live.panels.overlay = Some(screen),
        _ => live.panels.screen = screen,
    }
    if screen == Screen::Settings && !live.panels.directory_loaded {
        live.panels.directory = TextEdit::new(&live.app.preferences.worktree_directory);
        live.panels.directory_loaded = true;
    }
    let Some(project) = live.app.active_project.clone() else {
        return;
    };
    match screen {
        Screen::Files => {
            live.app.refresh_files();
            if let Some(saved) = live.app.preferences.files.get(&project).cloned() {
                for path in saved.expanded {
                    live.app.task(Task::Directory {
                        project: project.clone(),
                        path,
                        offset: 0,
                    });
                }
                if let Some(path) = saved
                    .selected
                    .filter(|path| !saved.scroll.contains_key(path))
                    && let Some(line) = saved.line.get(&path)
                {
                    live.panels.pending_line = Some((project, path, *line));
                }
            }
        }
        Screen::Changes => live.app.task(Task::Git { project }),
        Screen::Worktrees => live.app.task(Task::Worktrees {
            project,
            directory: live.app.preferences.worktree_directory.clone(),
        }),
        _ => {}
    }
}

pub fn open_link(s: &mut Workspace, index: usize, url: &str, tx: &mut Tx) {
    if url.starts_with("https://") || url.starts_with("http://") {
        let _ = tx.open_url(url);
        return;
    }
    let project = live(s).app.sessions[index].scope.project.clone();
    let (path, line) = url
        .split_once("#L")
        .or_else(|| {
            url.rsplit_once(':')
                .filter(|(_, n)| n.parse::<usize>().is_ok())
        })
        .map_or((url, None), |(path, line)| {
            (
                path,
                line.split('-').next().and_then(|v| v.parse::<usize>().ok()),
            )
        });
    let path = path.strip_prefix("file://").unwrap_or(path);
    let path = std::path::Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(&project).ok()
    } else {
        Some(path)
    };
    if let Some(path) = relative.filter(|p| {
        !p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    }) {
        let live = live_mut(s);
        live.app.select(index);
        let path = path.to_string_lossy().into_owned();
        live.panels.pending_line = line.map(|line| (project.clone(), path.clone(), line));
        live.app.open_file(project, path);
        live.panels.inspector = Screen::Files;
        live.panels.preview_page = line.unwrap_or(1).saturating_sub(1) / 200;
        s.inspector.set_open(true);
    }
}

pub fn navigation(_: &Workspace, k: &Kit) -> El {
    quiet_button(&ui::BRANCH, "New session in a project or worktree · ⇧⌘N")
        .on_press(open_launcher)
        .build(k)
        .key("nav-worktrees")
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if live(s).panels.overlay.is_some() {
        if live(s).panels.overlay == Some(Screen::Timeline) && matches!(ev.key, Key::Up | Key::Down)
        {
            return crate::commands::timeline_key(s, ev, tx);
        }
        if live(s).panels.overlay == Some(Screen::Sessions)
            && matches!(ev.key, Key::Up | Key::Down | Key::Enter)
        {
            return crate::commands::sessions_key(s, ev, tx);
        }
        if ev.key == Key::Escape {
            live_mut(s).panels.overlay = None;
            live_mut(s).focus();
        }
        return ['n', 'p', 'o', 'l', 'k', ',', 'i']
            .iter()
            .any(|c| ev.is_command(*c))
            || ev.key == Key::Escape;
    }
    if ev.is_command('f')
        && !ev.mods.shift
        && s.inspector.is_open()
        && live(s).panels.inspector == Screen::Files
    {
        let l = live_mut(s);
        l.panels.finding = true;
        l.panels.find.focused = true;
    } else if ev.is_command('g') && live(s).panels.finding {
        find_next(s, ev.mods.shift, tx);
    } else if ev.is_command('k') {
        s.sidebar.set_open(true);
        live_mut(s).panels.searching_sessions = true;
        live_mut(s).panels.sessions.focused = true;
    } else if ev.is_command('p') || (ev.is_command('f') && ev.mods.shift) {
        show(s, Screen::Files);
        live_mut(s).panels.content_search = ev.is_command('f');
        live_mut(s).panels.search.focused = true;
    } else if ev.is_command(',') {
        show(s, Screen::Settings);
    } else if ev.key == Key::Escape && live(s).panels.finding {
        live_mut(s).panels.finding = false;
        live_mut(s).panels.find.focused = false;
        live_mut(s).focus();
    } else if ev.key == Key::Escape && live(s).panels.searching_sessions {
        live_mut(s).panels.sessions.clear();
        live_mut(s).panels.sessions.focused = false;
        live_mut(s).panels.searching_sessions = false;
    } else if ev.key == Key::Escape && live(s).panels.screen == Screen::Review {
        live_mut(s).panels.screen = Screen::Conversation;
    } else {
        return false;
    }
    true
}

pub fn view(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut surface = col().w_full().grow(1.0).min_h(0.0).gap(10.0).padding(16.0);
    if let Some(error) = &live(s).app.persistence_error {
        surface = surface.child(label(error, k));
    }
    if let Some(project) = &live(s).app.active_project
        && let Some(error) = live(s)
            .app
            .services
            .checkouts
            .get(project)
            .and_then(|c| c.error.as_ref())
    {
        surface = surface.child(label(error, k));
    }
    surface.child(review(s, k, layers))
}

pub fn inspector(s: &Workspace, k: &Kit) -> El {
    let mut tabs = row().w_full().h(44.0).px(8.0).center_y().gap(4.0);
    for (name, screen) in [("Files", Screen::Files), ("Changes", Screen::Changes)] {
        let selected = live(s).panels.inspector == screen;
        let tab = ui::tab(name, selected, k)
            .on_press(move |s: &mut Workspace| show(s, screen))
            .h(30.0)
            .key(format!("nav-{}", name.to_lowercase()));
        tabs = tabs.child(tab);
    }
    let mut content = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(k.c.bg_base.mix(k.c.bg_deep, 0.55))
        .border(1.0, k.c.border_base)
        .child(
            tabs.child(rune::el().grow(1.0)).child(
                quiet_button(&glyphs::CLOSE, "Hide inspector")
                    .on_press(|s: &mut Workspace| s.inspector.set_open(false))
                    .build(k)
                    .key("close-inspector"),
            ),
        )
        .child(rune::el().w_full().h(1.0).fill(k.c.border_base));
    if let Some(project) = &live(s).app.active_project
        && let Some(error) = live(s)
            .app
            .services
            .checkouts
            .get(project)
            .and_then(|c| c.error.as_ref())
    {
        content = content.child(ui::caption(error, k).color(k.c.danger.fg).padding(12.0));
    }
    content.child(if live(s).panels.inspector == Screen::Changes {
        changes(s, k)
    } else {
        files(s, k).padding(12.0)
    })
}

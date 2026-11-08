//! Native workspace views; all reads, mutations and response ownership are in core.
use crate::ui::Button;
use crate::{
    Workspace,
    live::{live, live_mut},
    quiet_button, theme, ui,
};
use caelum_core::application::{Attachment, Task, Workspace as Application};
use caelum_protocol::{GitHubFeedbackKind, ProjectEntryKind, ProjectSearchMode};
use rune::{El, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text};
use rune_kit::{
    ButtonVariant, Dialog, Kit, Markdown, ScrollBar,
    diff::{self, DiffMode, DiffState, DiffView},
    glyphs,
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
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
    Timeline,
    Usage,
    Stats,
    Image,
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

pub fn open_launcher(s: &mut Workspace) {
    let l = live_mut(s);
    l.panels.launch_provider = l
        .app
        .selected
        .map(|i| l.app.sessions[i].scope.provider)
        .unwrap_or(l.app.preferences.default_provider);
    l.panels.overlay = Some(Screen::Launcher);
    l.commands.search.clear();
    l.commands.search.focused = true;
}
fn launcher(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let mut body = col().w_full().h_full().min_h(0.0).gap(12.0);
    let mut providers = row().gap(8.0);
    for provider in caelum_core::application::Provider::ALL {
        providers = providers.child(
            ui::tab(
                ui::provider_name(provider),
                l.panels.launch_provider == provider,
                k,
            )
            .on_press(move |s: &mut Workspace| live_mut(s).panels.launch_provider = provider)
            .key(format!("launch-provider-{}", provider.as_str())),
        );
    }
    body = body.child(providers).child(
        ui::field(
            &l.commands.search,
            "Find project",
            "Find a project or checkout…",
            k,
        )
        .key("launch-search")
        .on_edit(|s: &mut Workspace| &mut live_mut(s).commands.search),
    );
    let query = l.commands.search.text.to_lowercase();
    let mut projects = col().w_full().gap(5.0);
    for project in l.app.projects.iter().filter(|p| {
        format!("{} {}", p.name, p.path)
            .to_lowercase()
            .contains(&query)
    }) {
        let path = project.path.clone();
        let tree_project = path.clone();
        let branch = l
            .app
            .services
            .checkouts
            .get(&path)
            .and_then(|c| c.git.as_ref())
            .and_then(|g| g.branch.as_deref());
        projects = projects.child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(
                    ui::choice(
                        &branch.map_or_else(
                            || project.name.clone(),
                            |b| format!("{} · {b}", project.name),
                        ),
                        &project.path,
                        l.app.active_project.as_ref() == Some(&path),
                        k,
                    )
                    .on_press(move |s: &mut Workspace| {
                        let l = live_mut(s);
                        l.app.select_project(&path);
                        let scope = caelum_core::application::Scope {
                            project: path.clone(),
                            provider: l.panels.launch_provider,
                        };
                        l.app.project_draft(scope);
                        l.sync_editors();
                        l.panels.overlay = None;
                        l.panels.screen = Screen::Conversation;
                        l.focus();
                    })
                    .grow(1.0)
                    .min_w(0.0)
                    .key(crate::code_view::source_key(&project.path, "launch")),
                )
                .child(
                    quiet_button(&ui::BRANCH, "Create worktree for this project")
                        .on_press(move |s: &mut Workspace| {
                            crate::worktree::open(
                                s,
                                tree_project.clone(),
                                live(s).panels.launch_provider,
                            )
                        })
                        .build(k)
                        .key(crate::code_view::source_key(
                            &project.path,
                            "launch-worktree",
                        )),
                ),
        );
    }
    body.child(scroll(projects)).child(ui::divider(k)).child(
        row()
            .gap(8.0)
            .child(
                Button::new("Open folder…")
                    .on_press_tx(crate::live::open_project)
                    .build(k),
            )
            .child(
                Button::new("Create worktree…")
                    .disabled(l.app.active_project.is_none())
                    .on_press(|s: &mut Workspace| {
                        if let Some(project) = live(s).app.active_project.clone() {
                            crate::worktree::open(s, project, live(s).panels.launch_provider);
                        }
                    })
                    .build(k),
            ),
    )
}

pub fn open_review(s: &mut Workspace, project: String, url: String) {
    let live = live_mut(s);
    let key = (project.clone(), url.clone());
    if live.panels.review.as_ref() != Some(&key) {
        live.panels.review_restore = None;
        if let Some(previous) = live.panels.review.take() {
            live.panels.review_cache.insert(
                previous,
                (
                    std::mem::take(&mut live.panels.diff),
                    live.panels.diff_hash,
                    live.panels.annotations_hash,
                    live.panels.review_file.take(),
                ),
            );
        }
        let (diff, hash, notes, file) = live.panels.review_cache.remove(&key).unwrap_or_default();
        live.panels.diff = diff;
        live.panels.diff_hash = hash;
        live.panels.annotations_hash = notes;
        live.panels.review_file = file;
    }
    live.panels.screen = Screen::Review;
    live.panels.review = Some((project.clone(), url.clone()));
    live.app.task(Task::Details { project, url });
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if live(s).panels.overlay.is_some() {
        if live(s).panels.overlay == Some(Screen::Timeline) && matches!(ev.key, Key::Up | Key::Down)
        {
            return crate::commands::timeline_key(s, ev, tx);
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
    } else if ev.key == Key::Escape && live(s).panels.screen != Screen::Conversation {
        live_mut(s).panels.screen = Screen::Conversation;
    } else {
        return false;
    }
    true
}

pub fn view(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let mut surface = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(10.0)
        .padding(16.0)
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .child(ui::caption(
                    live(s)
                        .panels
                        .review
                        .as_ref()
                        .map(|(p, _)| {
                            format!("{} / Pull requests", p.rsplit('/').next().unwrap_or(p))
                        })
                        .unwrap_or_else(|| "Pull requests".into()),
                    k,
                ))
                .child(
                    Button::new("Back to session")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(|s: &mut Workspace| {
                            live_mut(s).panels.screen = Screen::Conversation
                        })
                        .build(k)
                        .key("back-to-session"),
                ),
        );
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

pub fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let Some(screen) = live(s).panels.overlay else {
        return;
    };
    let mut menus = Vec::new();
    let (title, description, body) = match screen {
        Screen::Settings => (
            "Settings",
            "Make Caelum feel like home.",
            crate::settings::view(s, k, &mut menus),
        ),
        Screen::Worktrees => (
            "Worktrees",
            "Independent checkouts for parallel work.",
            worktrees(s, k),
        ),
        Screen::CreateWorktree => (
            "Create worktree",
            "A fresh checkout for an independent session.",
            crate::worktree::view(s, k, &mut menus),
        ),
        Screen::Shells => (
            "Background commands",
            "Live output from this session.",
            crate::shells::panel(s, k),
        ),
        Screen::Timeline => (
            "Timeline",
            "Find and reveal an earlier prompt.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Usage => (
            "OpenCode Go usage",
            "Subscription allowances.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Stats => (
            "Statistics",
            "Tokens, cost and activity across recorded history.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Image => (
            "Image preview",
            "Fit, zoom and scroll the image.",
            crate::media::view(s, k),
        ),
        Screen::Launcher => (
            "New session",
            "Choose a project and provider.",
            launcher(s, k),
        ),
        _ => return,
    };
    let (width, height) = match screen {
        Screen::Settings => (760.0, 460.0),
        Screen::Launcher => (520.0, 310.0),
        Screen::CreateWorktree => (500.0, 330.0),
        Screen::Timeline => (580.0, 344.0),
        Screen::Stats => (720.0, 620.0),
        Screen::Image => (760.0, 480.0),
        _ => (660.0, 420.0),
    };
    let available =
        rune::memo::window_layout("shell").map_or(height, |r| (r[3] - 170.0).max(220.0));
    Dialog::new("workspace", title)
        .description(description)
        .open(true)
        .click_outside(false)
        .width(width)
        .fit()
        .on_close(|s: &mut Workspace| {
            live_mut(s).panels.overlay = None;
            live_mut(s).focus();
        })
        .child(
            col()
                .w_full()
                .h(height.min(available))
                .min_h(0.0)
                .px(24.0)
                .pb(24.0)
                .child(body),
        )
        .build(k, layers);
    layers.extend(menus);
}

fn run_search(s: &mut Workspace, _: &mut Tx) {
    let live = live_mut(s);
    let Some(project) = live.app.active_project.clone() else {
        return;
    };
    live.app.task(Task::Search {
        project,
        query: live.panels.search.text.clone(),
        mode: if live.panels.content_search {
            ProjectSearchMode::Content
        } else {
            ProjectSearchMode::Files
        },
    });
}

fn files(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(project) = &live.app.active_project else {
        return label("Open a project first", k);
    };
    let mut browser = col().w_full().gap(4.0).child(
        row()
            .gap(4.0)
            .center_y()
            .w_full()
            .child(
                ui::field(
                    &live.panels.search,
                    "Find files or text",
                    "Search files or text…",
                    k,
                )
                .key("file-search")
                .grow(1.0)
                .min_w(0.0)
                .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.search)
                .on_submit(run_search),
            )
            .child(
                quiet_button(&glyphs::SEARCH, "Find · Enter")
                    .on_press_tx(run_search)
                    .build(k)
                    .key("find-files"),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh files")
                    .on_press(|s: &mut Workspace| live_mut(s).app.refresh_files())
                    .build(k),
            ),
    );
    browser = browser.child(
        row()
            .w_full()
            .gap(4.0)
            .child(
                ui::tab("Paths", !live.panels.content_search, k)
                    .h(26.0)
                    .on_press(|s: &mut Workspace| live_mut(s).panels.content_search = false),
            )
            .child(
                ui::tab("Content", live.panels.content_search, k)
                    .h(26.0)
                    .on_press(|s: &mut Workspace| live_mut(s).panels.content_search = true),
            )
            .child(rune::el().grow(1.0)),
    );
    let Some(checkout) = live.app.services.checkouts.get(project) else {
        return browser.child(ui::skeleton("Loading files", 4, k));
    };
    let mut listing = col().w_full().gap(2.0);
    if !live.panels.search.text.is_empty() {
        if let Some(results) = &checkout.search {
            for hit in &results.matches {
                let project = project.clone();
                let path = hit.path.clone();
                let line = hit.line;
                listing = listing.child(
                    ui::choice(
                        &format!(
                            "{}{}",
                            hit.path,
                            hit.line.map(|l| format!(":{l}")).unwrap_or_default()
                        ),
                        hit.preview.as_deref().unwrap_or("Open file"),
                        false,
                        k,
                    )
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.panels.pending_line = line.map(|l| (project.clone(), path.clone(), l));
                        live.panels.selection = None;
                        live.app.open_file(project.clone(), path.clone());
                    }),
                );
            }
            if results.truncated || results.skipped > 0 {
                listing = listing.child(label(
                    format!("Bounded results · {} files skipped", results.skipped),
                    k,
                ));
            }
        }
    } else {
        listing = listing.child(directory(s, k, project, "", 0));
    }
    browser = browser.child(
        col()
            .w_full()
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(listing)
            .h(140.0)
            .key("file-list")
            .scroll_bar(k),
    );
    let mut tabs = row()
        .key("file-tabs")
        .role("tablist")
        .label("Open files")
        .focus_group()
        .focus_follows()
        .w_full()
        .scroll_x();
    for path in &checkout.tabs {
        let project = project.clone();
        let path = path.clone();
        let close_path = path.clone();
        let close_project = project.clone();
        let selected = checkout.selected_file.as_ref() == Some(&path);
        let key = format!("file-tab-{}", crate::code_view::source_key(&project, &path));
        let close_key = format!("close-{key}");
        let focused = live
            .focus_key
            .as_ref()
            .is_some_and(|f| f == &key || f == &close_key);
        let name = path.rsplit('/').next().unwrap_or(&path);
        let width = (rune::text::measure(name, &[], k.ty.ui, None).0 + 62.0).clamp(100.0, 210.0);
        let close = quiet_button(&glyphs::CLOSE, "Close file")
            .on_press(move |s: &mut Workspace| {
                let c = live_mut(s)
                    .app
                    .services
                    .checkouts
                    .get_mut(&close_project)
                    .unwrap();
                c.tabs.retain(|p| p != &close_path);
                if c.selected_file.as_ref() == Some(&close_path) {
                    c.selected_file = c.tabs.last().cloned();
                }
            })
            .build(k)
            .key(close_key)
            .label(format!("Close {path}"))
            .size(22.0, 24.0)
            .group_opacity(if focused { 1.0 } else { 0.0 }, 1.0);
        tabs = tabs.child(
            k.interactive(
                row()
                    .h(34.0)
                    .w(width)
                    .px(8.0)
                    .gap(6.0)
                    .center_y()
                    .fill(if selected {
                        k.c.layer_01
                    } else {
                        rune::Color::TRANSPARENT
                    })
                    .child(rune::icon(&ui::FILE, 13.0).color(if selected {
                        k.c.text_accent
                    } else {
                        k.c.icon_muted
                    }))
                    .child(
                        text(name)
                            .font(k.ty.ui)
                            .color(if selected {
                                k.c.text_base
                            } else {
                                k.c.text_muted
                            })
                            .truncate()
                            .min_w(0.0)
                            .grow(1.0),
                    )
                    .child(close)
                    .children(selected.then(|| {
                        rune::el().h(2.0).fill(k.c.text_accent).pin(
                            Some(0.0),
                            None,
                            Some(0.0),
                            Some(0.0),
                        )
                    })),
            )
            .key(key)
            .role("tab")
            .label(&path)
            .help(&path)
            .hover_group()
            .focus_group()
            .focus_return(if checkout.tabs.len() == 1 {
                "find-files"
            } else {
                "file-tabs"
            })
            .focusable(true)
            .focus_selected(selected)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let l = live_mut(s);
                l.panels.preview_page = 0;
                l.panels.selection = None;
                l.app.open_file(project.clone(), path.clone());
            }),
        );
    }
    if !checkout.tabs.is_empty() {
        browser = browser.child(tabs).child(ui::divider(k));
    }
    let Some(path) = &checkout.selected_file else {
        if live.app.services.busy(&Task::Directory {
            project: project.clone(),
            path: String::new(),
            offset: 0,
        }) && !checkout.directories.contains_key("")
        {
            return browser.grow(1.0);
        }
        return browser
            .child(ui::empty(
                "Explore your project",
                "Choose a file to read, search or attach.",
                k,
            ))
            .grow(1.0);
    };
    let Some(file) = checkout.files.get(path) else {
        return browser
            .child(ui::skeleton("Loading preview", 4, k))
            .grow(1.0);
    };
    let project_attach = project.clone();
    let path_attach = path.clone();
    let has_selection = live
        .panels
        .selection
        .as_ref()
        .is_some_and(|selection| &selection.project == project && &selection.path == path);
    let mut actions = row().w_full().gap(4.0).center_y().child(
        Button::new(if has_selection {
            "Add selection"
        } else {
            "Add to prompt"
        })
        .variant(ButtonVariant::GhostMuted)
        .icon(&crate::PLUS)
        .disabled(live.app.selected.is_none_or(|i| {
            live.app.sessions[i].scope.project != *project
                || live.app.sessions[i].archived
                || live.app.sessions[i].forgotten
        }))
        .on_press(move |s: &mut Workspace| {
            let live = live_mut(s);
            let Some(index) = live
                .app
                .selected
                .filter(|i| live.app.sessions[*i].scope.project == project_attach)
            else {
                return;
            };
            let Some(file) = live
                .app
                .services
                .checkouts
                .get(&project_attach)
                .and_then(|c| c.files.get(&path_attach))
            else {
                return;
            };
            let selected = live.panels.selection.as_ref().filter(|selection| {
                selection.project == project_attach && selection.path == path_attach
            });
            let range = selected.and_then(|selection| selection.lines);
            let attachment = Attachment::new(
                file.path.clone(),
                selected
                    .map(|selection| selection.text.clone())
                    .unwrap_or_else(|| file.content.clone()),
                range,
                file.truncated,
            );
            live.app.attach(index, attachment);
        })
        .build(k)
        .h(28.0)
        .px(6.0)
        .key("attach-preview"),
    );
    if file.truncated {
        browser = browser.child(ui::caption("Partial file · preview size limit reached", k));
    }
    let refresh = Task::File {
        project: project.clone(),
        path: path.clone(),
    };
    actions = actions.child(rune::el().grow(1.0)).child(
        quiet_button(&ui::REFRESH, "Refresh preview")
            .disabled(live.app.services.busy(&refresh))
            .on_press(move |s: &mut Workspace| live_mut(s).app.task(refresh.clone()))
            .build(k)
            .key("refresh-file-preview"),
    );
    let owner = (project.clone(), path.clone());
    if !live.panels.documents.contains_key(&owner) {
        return browser.child(label("Preparing file…", k)).grow(1.0);
    }
    actions = actions.child(
        quiet_button(&glyphs::SEARCH, "Find in file · ⌘F")
            .on_press(|s: &mut Workspace| {
                let p = &mut live_mut(s).panels;
                p.finding = true;
                p.find.focused = true;
            })
            .build(k)
            .key("find-in-file"),
    );
    browser = browser.child(actions);
    if live.panels.finding {
        browser = browser.child(
            row()
                .w_full()
                .gap(4.0)
                .child(
                    ui::field(&live.panels.find, "Find in file", "Find in file…", k)
                        .key("local-file-search")
                        .label("Find in file")
                        .placeholder("Find in file…", k.c.text_muted)
                        .color(k.c.text_base)
                        .single_line()
                        .grow(1.0)
                        .min_w(0.0)
                        .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.find)
                        .on_input_change(|s: &mut Workspace, tx| {
                            live_mut(s).panels.find_index = usize::MAX;
                            find_next(s, false, tx);
                        })
                        .on_submit(|s: &mut Workspace, tx| find_next(s, false, tx)),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_UP, "Previous match")
                        .on_press_tx(|s: &mut Workspace, tx| find_next(s, true, tx))
                        .build(k),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_DOWN, "Next match")
                        .on_press_tx(|s: &mut Workspace, tx| find_next(s, false, tx))
                        .build(k),
                ),
        );
    }
    let get = owner.clone();
    let edit = owner.clone();
    let keyboard = owner.clone();
    let selected = owner.clone();
    let viewer = DiffView::new(
        move |s: &Workspace| &crate::live::live(s).panels.documents[&get].diff,
        move |s: &mut Workspace| {
            &mut s
                .live
                .as_mut()
                .unwrap()
                .panels
                .documents
                .get_mut(&edit)
                .unwrap()
                .diff
        },
    )
    .key(crate::code_view::source_key(project, path))
    .on_select(move |s: &mut Workspace, range, _| {
        let l = live_mut(s);
        l.panels.selection = range.map(|r| {
            let first = r.start.line.min(r.end.line) as usize;
            let last = r.start.line.max(r.end.line) as usize;
            let source = &l.app.services.checkouts[&selected.0].files[&selected.1].content;
            Selection {
                project: selected.0.clone(),
                path: selected.1.clone(),
                text: source
                    .split_inclusive('\n')
                    .skip(first.saturating_sub(1))
                    .take(last - first + 1)
                    .collect(),
                lines: Some((first, last)),
            }
        });
    })
    .build(s, k)
    .focusable(true)
    .on_key(move |s: &mut Workspace, ev, tx| {
        let p = &mut live_mut(s).panels;
        let handled =
            crate::code_view::key(&mut p.documents.get_mut(&keyboard).unwrap().diff, ev, tx);
        if handled && ev.key == Key::Escape {
            p.selection = None;
        }
        handled
    })
    .on_selection_change(move |s: &mut Workspace, change, _| {
        if let Some(selection) = change.selection() {
            live_mut(s).panels.selection = Some(Selection {
                project: owner.0.clone(),
                path: owner.1.clone(),
                text: selection.text().into(),
                lines: None,
            });
        } else {
            live_mut(s).panels.selection = None;
        }
    });
    let selection = live
        .panels
        .selection
        .as_ref()
        .filter(|s| &s.project == project && &s.path == path);
    let viewer = crate::context::file(
        viewer,
        project.clone(),
        path.clone(),
        selection
            .map(|s| s.text.clone())
            .unwrap_or_else(|| file.content.clone()),
        selection.and_then(|s| s.lines),
    );
    live.panels.documents[&(project.clone(), path.clone())]
        .position
        .capture(&crate::code_view::source_key(project, path));
    browser.child(viewer).grow(1.0).min_h(0.0)
}

fn find_next(s: &mut Workspace, backwards: bool, tx: &mut Tx) {
    let l = live_mut(s);
    let Some(project) = l.app.active_project.clone() else {
        return;
    };
    let Some(checkout) = l.app.services.checkouts.get(&project) else {
        return;
    };
    let Some(path) = checkout.selected_file.clone() else {
        return;
    };
    let Some(file) = checkout.files.get(&path) else {
        return;
    };
    let query = l.panels.find.text.to_lowercase();
    if query.is_empty() {
        return;
    }
    let lines: Vec<_> = file
        .content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(&query))
        .map(|(i, _)| i + 1)
        .collect();
    if lines.is_empty() {
        return;
    }
    let next = if l.panels.find_index == usize::MAX {
        0
    } else if backwards {
        (l.panels.find_index + lines.len() - 1) % lines.len()
    } else {
        (l.panels.find_index + 1) % lines.len()
    };
    l.panels.find_index = next;
    if let Some(document) = l.panels.documents.get_mut(&(project, path)) {
        let line = diff::LineRef::new(diff::Side::New, lines[next] as u32);
        document
            .diff
            .set_selection(Some(diff::LineRange::line(0, line)));
        document.diff.scroll_to_line(0, line, tx);
    }
}

fn directory(s: &Workspace, k: &Kit, project: &str, path: &str, depth: usize) -> El {
    let mut tree = col().w_full().gap(2.0);
    let Some(listing) = live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.directories.get(path))
    else {
        return if live(s).app.services.busy(&Task::Directory {
            project: project.into(),
            path: path.into(),
            offset: 0,
        }) {
            tree.child(ui::skeleton("Loading files", 2, k))
        } else {
            tree
        };
    };
    for entry in &listing.entries {
        let dir = entry.kind == ProjectEntryKind::Directory;
        let p = project.to_owned();
        let f = entry.path.clone();
        let expanded = live(s)
            .app
            .preferences
            .files
            .get(&p)
            .is_some_and(|saved| saved.expanded.contains(&f));
        tree = tree.child(
            k.interactive(
                row()
                    .w_full()
                    .h(28.0)
                    .pl(6.0 + depth as f32 * 12.0)
                    .pr(6.0)
                    .gap(7.0)
                    .radius(6.0)
                    .center_y()
                    .child(
                        rune::icon(
                            if dir {
                                if expanded {
                                    &glyphs::CHEVRON_DOWN
                                } else {
                                    &glyphs::CARET_RIGHT
                                }
                            } else {
                                &ui::FILE
                            },
                            13.0,
                        )
                        .color(k.c.icon_muted),
                    )
                    .child(
                        text(&entry.name)
                            .font(k.ty.ui)
                            .color(k.c.text_base)
                            .truncate()
                            .grow(1.0),
                    ),
            )
            .role("button")
            .label(&entry.path)
            .focusable(true)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                if dir {
                    let saved = live.app.preferences.files.entry(p.clone()).or_default();
                    if !saved.expanded.remove(&f) {
                        saved.expanded.insert(f.clone());
                        live.app.task(Task::Directory {
                            project: p.clone(),
                            path: f.clone(),
                            offset: 0,
                        });
                    }
                } else {
                    live.panels.preview_page = 0;
                    live.panels.selection = None;
                    live.app.open_file(p.clone(), f.clone());
                }
            })
            .key(format!("file-tree-{}", entry.path)),
        );
        if dir && expanded && depth < 30 {
            tree = tree.child(directory(s, k, project, &entry.path, depth + 1));
        }
    }
    if let Some(offset) = listing.next_offset {
        tree = tree.child(action(
            "More files",
            Task::Directory {
                project: project.into(),
                path: path.into(),
                offset,
            },
            s,
            k,
        ));
    }
    tree
}

fn diff_view(s: &Workspace, k: &Kit, review: bool) -> El {
    if !review
        && let Some(position) = live(s)
            .panels
            .change_file
            .as_ref()
            .and_then(|key| live(s).panels.change_positions.get(key))
    {
        position.capture("changes-diff");
    }
    if review
        && let Some(key) = &live(s).panels.review
        && let Some(position) = live(s).panels.review_positions.get(key)
    {
        position.capture("diff");
    }
    let kit = k.clone();
    let action_kit = k.clone();
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(
            Button::new(
                if (if review {
                    &live(s).panels.diff
                } else {
                    &live(s).panels.change_diff
                })
                .mode()
                    == DiffMode::Unified
                {
                    "Split diff"
                } else {
                    "Unified diff"
                },
            )
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let panels = &mut live_mut(s).panels;
                let diff = if review {
                    &mut panels.diff
                } else {
                    &mut panels.change_diff
                };
                diff.set_mode(if diff.mode() == DiffMode::Unified {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                });
            })
            .build(k)
            .self_start(),
        )
        .child(
            DiffView::new(
                move |s: &Workspace| {
                    if review {
                        &live(s).panels.diff
                    } else {
                        &live(s).panels.change_diff
                    }
                },
                move |s: &mut Workspace| {
                    if review {
                        &mut s.live.as_mut().unwrap().panels.diff
                    } else {
                        &mut s.live.as_mut().unwrap().panels.change_diff
                    }
                },
            )
            .key(if review { "diff" } else { "changes-diff" })
            .header_actions(move |s, i| {
                let l = live(s);
                let state = if review {
                    &l.panels.diff
                } else {
                    &l.panels.change_diff
                };
                let file = &state.files()[i];
                let path = file.path().to_owned();
                let Some(project) = (if review {
                    l.panels.review.as_ref().map(|v| v.0.clone())
                } else {
                    l.panels.change_file.as_ref().map(|v| v.0.clone())
                }) else {
                    return col();
                };
                if review
                    && let Some((key, file)) = l.panels.review.as_ref().and_then(|key| {
                        l.app
                            .services
                            .reviews
                            .get(key)?
                            .files
                            .iter()
                            .find(|f| f.path == path)
                            .map(|file| (key, file))
                    })
                    && file.patch.is_none()
                {
                    let url = format!("{}/files", key.1);
                    return Button::new("Patch unavailable")
                        .icon(&ui::EXTERNAL)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&url);
                        })
                        .build(&action_kit)
                        .help("Binary or omitted by GitHub. Open GitHub for the full file.")
                        .key(format!("unavailable-patch-{path}"));
                }
                let selected = state.selection().filter(|selection| selection.file == i);
                let content = selected
                    .and_then(|_| state.selected_text())
                    .unwrap_or_else(|| {
                        if review {
                            l.panels
                                .review
                                .as_ref()
                                .and_then(|key| l.app.services.reviews.get(key))
                                .and_then(|r| r.files.iter().find(|f| f.path == path))
                                .and_then(|f| f.patch.clone())
                                .unwrap_or_default()
                        } else {
                            l.app
                                .services
                                .checkouts
                                .get(&project)
                                .and_then(|c| c.diffs.get(&path))
                                .map(|d| format!("{}{}{}", d.staged, d.unstaged, d.untracked))
                                .unwrap_or_default()
                        }
                    });
                let copy = content.clone();
                if content.is_empty() {
                    return col();
                }
                crate::context::file(
                    Button::new(if selected.is_some() {
                        "Copy selection"
                    } else {
                        "Copy patch"
                    })
                    .variant(ButtonVariant::GhostMuted)
                    .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                    .build(&action_kit),
                    project,
                    path,
                    content,
                    selected.map(|s| {
                        (
                            s.start.line.min(s.end.line) as usize,
                            s.start.line.max(s.end.line) as usize,
                        )
                    }),
                )
            })
            .annotation(move |s, annotation| {
                let Some((project, url)) = &live(s).panels.review else {
                    return col();
                };
                let Some(thread) = live(s)
                    .app
                    .services
                    .reviews
                    .get(&(project.clone(), url.clone()))
                    .and_then(|r| r.threads.iter().find(|t| t.id == annotation.id))
                else {
                    return col();
                };
                thread_view(s, &kit, project, url, thread)
            })
            .on_visible_file(move |s, index| {
                if review {
                    let l = s.live.as_mut().unwrap();
                    // A partial refresh page must not replace the file we are
                    // restoring with its temporary first visible file.
                    if l.panels.review_restore.is_none() {
                        l.panels.review_file =
                            l.panels.diff.files().get(index).map(|f| f.path().into());
                    }
                    if index + 3 >= l.panels.diff.files().len()
                        && let Some((project, url)) = l.panels.review.clone()
                        && let Some(r) = l.app.services.reviews.get(&(project.clone(), url.clone()))
                        && let (Some(page), Some(head)) = (r.next_page, r.head())
                    {
                        l.app.task(Task::ReviewFiles {
                            project,
                            url,
                            head: head.into(),
                            page,
                        });
                    }
                }
            })
            .build(s, k)
            .focusable(true)
            .on_key(move |s: &mut Workspace, ev, tx| {
                let p = &mut live_mut(s).panels;
                crate::code_view::key(
                    if review {
                        &mut p.diff
                    } else {
                        &mut p.change_diff
                    },
                    ev,
                    tx,
                )
            }),
        )
}

fn changes(s: &Workspace, k: &Kit) -> El {
    let Some(project) = &live(s).app.active_project else {
        return label("Open a project first", k);
    };
    let task = Task::Git {
        project: project.clone(),
    };
    let name = std::path::Path::new(project)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(project);
    let mut content = col().w_full().grow(1.0).min_h(0.0).gap(8.0).child(
        row()
            .w_full()
            .px(12.0)
            .pt(8.0)
            .center_y()
            .gap(7.0)
            .child(rune::icon(&ui::BRANCH, 16.0).color(k.c.icon_muted))
            .child(
                text(name)
                    .font(k.ty.ui)
                    .color(k.c.text_base)
                    .grow(1.0)
                    .truncate(),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh changes")
                    .disabled(live(s).app.services.busy(&task))
                    .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
                    .build(k)
                    .key("refresh-changes"),
            ),
    );
    if let Some(git) = live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.git.as_ref())
    {
        content = content.child(
            row()
                .w_full()
                .px(14.0)
                .center_y()
                .gap(6.0)
                .child(rune::icon(&ui::BRANCH, 12.0).color(k.c.icon_muted))
                .child(
                    ui::caption(
                        git.branch.as_deref().unwrap_or("Detached / non-repository"),
                        k,
                    )
                    .truncate()
                    .grow(1.0),
                )
                .child(ui::caption(
                    format!(
                        "{} {}",
                        git.changes.len(),
                        if git.changes.len() == 1 {
                            "file"
                        } else {
                            "files"
                        }
                    ),
                    k,
                )),
        );
        let mut files = col().w_full().px(8.0).gap(2.0);
        for change in &git.changes {
            let project = project.clone();
            let path = change.path.clone();
            let selected =
                live(s).panels.change_file.as_ref() == Some(&(project.clone(), path.clone()));
            files = files.child(
                k.interactive(
                    row()
                        .w_full()
                        .h(28.0)
                        .px(8.0)
                        .gap(8.0)
                        .center_y()
                        .radius(6.0)
                        .fill(if selected {
                            k.c.hover
                        } else {
                            rune::Color::TRANSPARENT
                        })
                        .child(
                            text(format!("{}{}", change.index_status, change.worktree_status))
                                .font(k.ty.small)
                                .color(k.c.warning.icon),
                        )
                        .child(
                            text(&path)
                                .font(rune::TextStyle {
                                    size: 11.0,
                                    ..k.ty.mono
                                })
                                .color(k.c.text_base)
                                .truncate()
                                .grow(1.0),
                        ),
                )
                .role("button")
                .label(&path)
                .focusable(true)
                .focus_ring(1.0, 0.0, k.c.border_focus)
                .on_press(move |s: &mut Workspace| {
                    select_change(s, project.clone(), path.clone());
                }),
            );
        }
        let height = (git.changes.len() as f32 * 28.0).clamp(28.0, 160.0);
        content = content.child(scroll(files).min_h(height).max_h(height));
        if git.changes.is_empty() {
            content = content.child(ui::empty(
                "Working tree clean",
                "Changes appear here as you and your agents edit.",
                k,
            ));
        }
    } else {
        return content.child(ui::skeleton("Loading changes", 4, k).padding(12.0));
    }
    if live(s)
        .panels
        .change_file
        .as_ref()
        .is_some_and(|(p, _)| p == project)
    {
        let ready = live(s).panels.change_file.as_ref().is_some_and(|(p, f)| {
            live(s)
                .app
                .services
                .checkouts
                .get(p)
                .is_some_and(|c| c.diffs.contains_key(f))
        });
        content = content.child(if ready {
            diff_view(s, k, false)
        } else {
            ui::skeleton("Loading patch", 5, k).padding(12.0)
        });
    } else if live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.git.as_ref())
        .is_some_and(|g| !g.changes.is_empty())
    {
        content = content.child(ui::caption("Select a file to review its patch", k).padding(16.0));
    }
    content
}

pub fn select_change(s: &mut Workspace, project: String, path: String) {
    let live = live_mut(s);
    if live.panels.change_file.as_ref() == Some(&(project.clone(), path.clone())) {
        return;
    }
    let position = live
        .panels
        .change_positions
        .entry((project.clone(), path.clone()))
        .or_default();
    live.panels.pending_scroll = Some((
        "changes-diff/scroll".into(),
        position.get().unwrap_or([0.0, 0.0]),
    ));
    live.panels.change_file = Some((project.clone(), path.clone()));
    live.panels.change_diff = DiffState::new(diff::parse(""));
    live.panels.change_hash = 0;
    live.app.task(Task::Diff { project, path });
}

fn worktrees(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(project) = &live.app.active_project else {
        return ui::empty(
            "Open a Git project",
            "Choose a project before creating a separate checkout.",
            k,
        );
    };
    let refresh = Task::Worktrees {
        project: project.clone(),
        directory: live.app.preferences.worktree_directory.clone(),
    };
    let mutation = Task::CreateWorktree {
        project: project.clone(),
        branch: String::new(),
        base: String::new(),
        directory: String::new(),
    };
    let busy = live.app.services.busy(&mutation);
    let checkout = live.app.services.checkouts.get(project);
    let mut content = col().w_full().gap(12.0).child(
        row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                ui::caption(project, k)
                    .truncate()
                    .grow(1.0)
                    .min_w(0.0)
                    .help(project),
            )
            .child(action("Refresh", refresh.clone(), s, k))
            .child(
                Button::new("Create worktree…")
                    .variant(ButtonVariant::Contrast)
                    .on_press({
                        let project = project.clone();
                        move |s: &mut Workspace| {
                            let l = crate::live::live(s);
                            let provider = l
                                .app
                                .selected
                                .map(|i| l.app.sessions[i].scope.provider)
                                .unwrap_or(l.app.preferences.default_provider);
                            crate::worktree::open(s, project.clone(), provider);
                        }
                    })
                    .build(k),
            ),
    );
    if let Some(error) = live
        .app
        .services
        .error(&mutation)
        .or_else(|| live.app.services.error(&refresh))
        .or_else(|| checkout.and_then(|c| c.error.as_deref()))
    {
        content = content.child(ui::notice("Worktree unavailable", error, k));
    }
    if let Some(listing) = live
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.worktrees.as_ref())
    {
        for tree in &listing.worktrees {
            let path = tree.project.path.clone();
            let label = tree.label.clone().unwrap_or_else(|| {
                std::path::Path::new(&tree.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            let detail = format!(
                "{}{}",
                tree.branch.as_deref().unwrap_or("Detached HEAD"),
                if tree.main { " · Main checkout" } else { "" }
            );
            let mut line = ui::card(k).padding(6.0).gap(4.0).child(
                ui::choice(&label, &detail, &tree.project.path == project, k)
                    .disabled(!tree.available || busy)
                    .opacity(if tree.available { 1.0 } else { 0.5 })
                    .help(format!("{}\n{}", detail, tree.project.path))
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.app.open_project(path.clone());
                        live.panels.overlay = None;
                    }),
            );
            if !tree.main {
                line = line.child(crate::worktree::rename_control(
                    s,
                    k,
                    project.clone(),
                    &tree.path,
                    &label,
                ));
            }
            if let Some(reason) = &tree.locked {
                line = line.child(ui::caption(format!("Locked · {reason}"), k).px(10.0));
            } else if !tree.available {
                line = line.child(ui::caption("Checkout directory is unavailable", k).px(10.0));
            }
            if !tree.main && tree.locked.is_none() {
                line = line.child(action(
                    "Remove checkout",
                    Task::RemoveWorktree {
                        project: project.clone(),
                        path: tree.path.clone(),
                    },
                    s,
                    k,
                ));
            }
            content = content.child(line);
        }
    } else {
        content = content.child(
            if live.app.services.busy(&Task::Worktrees {
                project: project.clone(),
                directory: live.app.preferences.worktree_directory.clone(),
            }) {
                ui::skeleton("Loading worktrees", 3, k)
            } else {
                ui::empty(
                    "No worktrees yet",
                    "Open a Git project to create an independent checkout.",
                    k,
                )
            },
        );
    }
    scroll(content.pr(10.0))
        .key("worktrees-scroll")
        .scroll_bar(k)
}

pub(crate) fn appearance_settings(s: &Workspace, k: &Kit) -> El {
    let preferences = &live(s).app.preferences;
    let mut content = col().w_full().gap(16.0).child(ui::section(
        "Appearance",
        "Choose a theme and an accent for your workspace.",
        k,
    ));
    let mut themes = row().w_full().gap(8.0);
    for (mode, name) in [("system", "System"), ("dark", "Dark"), ("light", "Light")] {
        themes = themes.child(
            ui::tab(name, preferences.theme == mode, k)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.preferences.theme = mode.into();
                    s.dark = None;
                })
                .key(format!("theme-{mode}"))
                .grow(1.0),
        );
    }
    content = content.child(themes);
    let mut families = row().w_full().flex_wrap().gap(8.0);
    for (id, name) in theme::FAMILIES {
        let palette = theme::appearance(k.dark, id, "theme");
        families = families.child(
            k.interactive(
                col()
                    .grow(1.0)
                    .min_w(100.0)
                    .gap(8.0)
                    .padding(10.0)
                    .radius(8.0)
                    .fill(k.c.bg_base)
                    .border(
                        1.0,
                        if preferences.theme_family == id {
                            k.c.border_focus
                        } else {
                            k.c.border_base
                        },
                    )
                    .child(
                        row()
                            .w_full()
                            .h(30.0)
                            .radius(5.0)
                            .fill(palette.c.bg_base)
                            .child(rune::el().w(22.0).h_full().fill(palette.c.bg_deep))
                            .child(
                                rune::el()
                                    .w(30.0)
                                    .h(5.0)
                                    .ml(8.0)
                                    .mt(12.0)
                                    .radius(2.0)
                                    .fill(palette.c.text_accent),
                            ),
                    )
                    .child(ui::caption(name, k)),
            )
            .role("button")
            .label(name)
            .focusable(true)
            .focus_ring(1.0, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let p = &mut live_mut(s).app.preferences;
                if id == "vercel" && p.theme_family != id {
                    p.accent_color = "theme".into();
                }
                p.theme_family = id.into();
            })
            .key(format!("family-{id}")),
        );
    }
    content = content
        .child(families)
        .child(ui::caption("Accent colour", k));
    let mut accents = row().gap(8.0);
    for accent in theme::ACCENTS {
        let palette = theme::appearance(k.dark, &preferences.theme_family, accent);
        accents = accents.child(
            k.interactive(
                rune::el()
                    .size(30.0, 30.0)
                    .radius(15.0)
                    .fill(palette.c.text_accent)
                    .border(
                        if preferences.accent_color == accent {
                            3.0
                        } else {
                            1.0
                        },
                        k.c.text_base,
                    ),
            )
            .role("button")
            .label(format!("{accent} accent"))
            .focusable(true)
            .focus_ring(2.0, 2.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.preferences.accent_color = accent.into()
            })
            .key(format!("accent-{accent}")),
        );
    }
    content.child(accents)
}

fn review(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let Some((project, url)) = &live(s).panels.review else {
        return label("Choose a pull request", k);
    };
    let Some(review) = live(s)
        .app
        .services
        .reviews
        .get(&(project.clone(), url.clone()))
    else {
        return ui::skeleton("Loading pull request", 6, k);
    };
    let mut content = col().w_full().gap(12.0);
    if let Some(error) = &review.error {
        content = content.child(
            ui::notice("Review unavailable", error, k).child(
                action(
                    "Retry",
                    Task::Details {
                        project: project.clone(),
                        url: url.clone(),
                    },
                    s,
                    k,
                )
                .self_start(),
            ),
        );
    }
    if let Some(notice) = &review.notice {
        content = content.child(label(notice, k));
    }
    let Some(details) = &review.details else {
        return scroll(if review.error.is_some() {
            content
        } else {
            content.child(ui::skeleton("Loading overview", 6, k))
        });
    };
    let head = &details.head_ref_oid;
    let mut tabs = row().w_full().gap(6.0);
    for (tab, name) in [
        "Overview".to_owned(),
        format!("Files changed ({})", details.changed_files),
    ]
    .into_iter()
    .enumerate()
    {
        tabs = tabs.child(
            ui::tab(&name, live(s).panels.review_tab == tab, k)
                .on_press(move |s: &mut Workspace| live_mut(s).panels.review_tab = tab)
                .key(format!("review-tab-{tab}")),
        );
    }
    let link = url.clone();
    let refresh_project = project.clone();
    let refresh_url = url.clone();
    content = content
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    ui::caption(
                        if details.pull_request.is_draft {
                            "Draft"
                        } else {
                            &details.state
                        },
                        k,
                    )
                    .color(match details.state.as_str() {
                        "MERGED" => k.c.text_accent,
                        "CLOSED" => k.c.danger.fg,
                        _ if details.pull_request.is_draft => k.c.text_muted,
                        _ => k.c.success.fg,
                    })
                    .px(8.0)
                    .py(4.0)
                    .radius(6.0)
                    .fill(k.c.layer_01)
                    .border(1.0, k.c.border_base),
                )
                .child(ui::caption(
                    format!(
                        "#{} · opened by {}",
                        details.pull_request.number,
                        details
                            .author
                            .as_ref()
                            .map(|a| a.login.as_str())
                            .unwrap_or("Unknown")
                    ),
                    k,
                ))
                .child(rune::el().grow(1.0))
                .child(
                    Button::new("GitHub")
                        .icon(&ui::EXTERNAL)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&link);
                        })
                        .build(k),
                )
                .child(
                    quiet_button(&ui::REFRESH, "Refresh review")
                        .on_press(move |s: &mut Workspace| {
                            let l = live_mut(s);
                            l.app
                                .services
                                .refresh_review(&refresh_project, &refresh_url);
                            l.app.task(Task::Details {
                                project: refresh_project.clone(),
                                url: refresh_url.clone(),
                            });
                        })
                        .build(k),
                ),
        )
        .child(
            text(&details.pull_request.title)
                .font(rune::TextStyle::new(22.0, 550.0))
                .color(k.c.text_base)
                .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(
                    text(&details.head_ref_name)
                        .font(k.ty.mono)
                        .color(k.c.text_accent)
                        .truncate()
                        .min_w(0.0)
                        .px(8.0)
                        .py(4.0)
                        .radius(5.0)
                        .fill(k.c.layer_02),
                )
                .child(rune::icon(&ui::BRANCH, 14.0).color(k.c.icon_muted))
                .child(
                    text(&details.pull_request.base_ref_name)
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .truncate()
                        .min_w(0.0)
                        .px(8.0)
                        .py(4.0)
                        .radius(5.0)
                        .fill(k.c.layer_02),
                )
                .child(rune::el().grow(1.0))
                .child(ui::caption(
                    format!("+{}  −{}", details.additions, details.deletions),
                    k,
                )),
        )
        .child(tabs)
        .child(ui::divider(k));
    let header = content;
    if live(s).panels.review_tab == 1 {
        return header
            .grow(1.0)
            .min_h(0.0)
            .child(review_files_view(s, k, project, url, review))
            .children(
                (!review.selected_comments.is_empty())
                    .then(|| review_actions(s, k, project, url, review)),
            );
    }
    let mut content = col().w_full().max_w(960.0).self_center().gap(24.0).pb(24.0);
    if live(s).panels.review_tab == 0 {
        content = content.child(
            col().w_full().py(8.0).child(
                Markdown::new(if details.body.trim().is_empty() {
                    "No description provided."
                } else {
                    &details.body
                })
                .selectable(true)
                .render(&theme::markdown(k)),
            ),
        );
    }
    content = content
        .child(crate::merge::view(s, k, project, url, details, layers))
        .child(
            text("Discussion & reviews")
                .font(k.ty.ui_medium)
                .color(k.c.text_base),
        );
    if review.discussion.is_empty() && review.reviews.is_empty() && review.threads.is_empty() {
        content = content.child(if review.cursors.len() < 3 && review.error.is_none() {
            ui::skeleton("Loading discussion", 3, k)
        } else {
            ui::caption(
                "No discussion yet. Review comments and replies will appear here.",
                k,
            )
            .py(12.0)
        });
    }
    let mut discussion = col().w_full().gap(10.0);
    for comment in review.discussion.iter().chain(&review.reviews) {
        discussion = discussion.child(comment_view(s, k, project, url, comment));
    }
    for thread in &review.threads {
        discussion = discussion.child(thread_view(s, k, project, url, thread));
    }
    content = content.child(discussion);
    for kind in [
        GitHubFeedbackKind::Discussion,
        GitHubFeedbackKind::Reviews,
        GitHubFeedbackKind::Threads,
    ] {
        if let Some(Some(cursor)) = review.cursors.get(&format!("{kind:?}")) {
            content = content.child(action(
                &format!("More {kind:?}"),
                Task::Feedback {
                    project: project.clone(),
                    url: url.clone(),
                    head: head.clone(),
                    kind,
                    cursor: Some(cursor.clone()),
                    thread: None,
                },
                s,
                k,
            ));
        }
    }
    header
        .max_w(960.0)
        .self_center()
        .grow(1.0)
        .min_h(0.0)
        .child(scroll(content).key("review-overview-scroll").scroll_bar(k))
        .children(
            (!review.selected_comments.is_empty())
                .then(|| review_actions(s, k, project, url, review)),
        )
}

fn review_actions(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    review: &caelum_core::application::Review,
) -> El {
    let index = live(s).app.selected;
    let destination = index
        .map(|i| live(s).app.sessions[i].title.as_str())
        .unwrap_or("No session selected");
    let blocker = index.map_or(Some("Choose a session in this checkout"), |i| {
        live(s).app.review_blocker(i, project)
    });
    let project = project.to_owned();
    let url = url.to_owned();
    let compact = rune::memo::window_layout("main-pane").is_some_and(|r| r[2] < 620.0);
    row()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .fill(k.c.layer_02)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(ui::caption(format!("Send to {destination}"), k).truncate())
                .child(
                    ui::caption(
                        blocker.unwrap_or("Selected feedback will start a new turn"),
                        k,
                    )
                    .truncate(),
                ),
        )
        .child(
            Button::new(if compact {
                format!(
                    "Address {} comment{}",
                    review.selected_comments.len(),
                    if review.selected_comments.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else {
                format!(
                    "Address {} selected comment{}",
                    review.selected_comments.len(),
                    if review.selected_comments.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            })
            .variant(ButtonVariant::Contrast)
            .disabled(blocker.is_some() || review.selected_comments.is_empty())
            .on_press_tx(move |s: &mut Workspace, tx| {
                let Some(index) = index else {
                    return;
                };
                let l = live_mut(s);
                if l.app.address_review(index, &project, &url) {
                    l.app.select(index);
                    l.panels.screen = Screen::Conversation;
                    l.focus();
                    tx.scroll_to_bottom_instant(format!("live-transcript-{index}"));
                }
            })
            .build(k)
            .key("address-comments"),
        )
}

fn review_files_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    review: &caelum_core::application::Review,
) -> El {
    let owner = (project.to_owned(), url.to_owned());
    let lookup = owner.clone();
    let kit = k.clone();
    let query = live(s).panels.review_search.text.to_lowercase();
    let indices: Vec<_> = review
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.path.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
    let keys = indices.clone();
    let files = col()
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            indices.len(),
            50.0,
            move |s: &Workspace, i| {
                format!(
                    "review-file-{}",
                    live(s).app.services.reviews[&owner].files[keys[i]].path
                )
                .into()
            },
            move |s: &Workspace, i| {
                let r = &live(s).app.services.reviews[&lookup];
                let f = &r.files[indices[i]];
                let path = f.path.clone();
                let count = r.threads.iter().filter(|t| t.path == path).count();
                let detail = format!(
                    "{} · +{} −{}{}{}",
                    f.status,
                    f.additions,
                    f.deletions,
                    if count > 0 {
                        format!(" · {count} comments")
                    } else {
                        String::new()
                    },
                    if f.patch.is_none() {
                        " · Patch unavailable"
                    } else {
                        ""
                    }
                );
                let mut item = col().w_full().child(
                    ui::choice(
                        &path,
                        &detail,
                        live(s).panels.review_file.as_ref() == Some(&path),
                        &kit,
                    )
                    .min_h(44.0)
                    .on_press_tx(move |s: &mut Workspace, tx| {
                        let p = &mut live_mut(s).panels;
                        p.review_restore = None;
                        p.review_file = Some(path.clone());
                        if let Some(index) = p.diff.file_index(&path) {
                            p.diff.scroll_to_file(index, tx);
                        }
                    })
                    .h(48.0)
                    .w_full(),
                );
                for thread in r
                    .threads
                    .iter()
                    .filter(|t| t.path == f.path && !t.is_resolved)
                    .take(3)
                {
                    let id = thread.id.clone();
                    item = item.child(
                        Button::new(format!(
                            "Line {} · {}",
                            thread.line.or(thread.original_line).unwrap_or(0),
                            thread
                                .comments
                                .first()
                                .and_then(|c| c.author.as_ref())
                                .map(|a| a.login.as_str())
                                .unwrap_or("Thread")
                        ))
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |s: &mut Workspace, tx| jump_thread(s, &id, tx))
                        .build(&kit)
                        .key(format!("thread-link-{}", thread.id))
                        .pl(12.0),
                    );
                }
                item
            },
        );
    let compact = rune::memo::window_layout("main-pane").is_some_and(|r| r[2] < 640.0);
    let show_nav = live(s).panels.review_nav.unwrap_or(!compact);
    let nav = col()
        .key("review-file-list")
        .min_h(0.0)
        .h_full()
        .gap(8.0)
        .child(
            ui::field(
                &live(s).panels.review_search,
                "Find changed file",
                "Find a file…",
                k,
            )
            .key("review-file-search")
            .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.review_search),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(files)
                .scroll_bar(k),
        );
    let nav = if compact {
        nav.w_full().h(126.0)
    } else {
        nav.w(224.0)
    };
    let mut code = if !review.files_loaded {
        ui::skeleton("Loading changed files", 6, k).grow(1.0)
    } else {
        diff_view(s, k, true)
    };
    if let Some(file) = review
        .files
        .iter()
        .find(|f| live(s).panels.review_file.as_ref() == Some(&f.path))
    {
        let mut fallback = col().w_full().gap(6.0);
        let mut has_fallback = false;
        if file.patch.is_none() {
            has_fallback = true;
            fallback = fallback.child(ui::caption(
                "Patch unavailable: binary or omitted by GitHub. Open GitHub for the full file.",
                k,
            ));
        }
        for thread in review.threads.iter().filter(|t| {
            t.path == file.path
                && (t.is_outdated
                    || t.line.is_none_or(|line| {
                        let side = if t.diff_side == "LEFT" {
                            diff::Side::Old
                        } else {
                            diff::Side::New
                        };
                        live(s).panels.diff.file_index(&file.path).is_none_or(|i| {
                            !crate::code_view::has_line(&live(s).panels.diff.files()[i], side, line)
                        })
                    }))
        }) {
            has_fallback = true;
            fallback = fallback.child(thread_view(s, k, project, url, thread));
        }
        if has_fallback {
            code = code.child(
                fallback
                    .max_h(220.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .key("review-file-notes"),
            );
        }
    }
    let mut toolbar = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            quiet_button(&rune_kit::shell::SIDEBAR, "Toggle changed files")
                .expanded(show_nav)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).panels.review_nav = Some(!show_nav);
                })
                .build(k)
                .key("review-nav-toggle"),
        )
        .child(
            ui::caption(
                live(s)
                    .panels
                    .review_file
                    .as_deref()
                    .unwrap_or("Changed files"),
                k,
            )
            .truncate()
            .grow(1.0)
            .min_w(0.0),
        );
    for (label, backwards) in [("Previous thread", true), ("Next thread", false)] {
        toolbar = toolbar.child(
            quiet_button(
                if backwards {
                    &ui::BACK
                } else {
                    &glyphs::CARET_RIGHT
                },
                label,
            )
            .disabled(review.threads.is_empty())
            .on_press_tx(move |s: &mut Workspace, tx| {
                let l = live(s);
                let Some(r) = l
                    .panels
                    .review
                    .as_ref()
                    .and_then(|key| l.app.services.reviews.get(key))
                else {
                    return;
                };
                let current = r.threads.iter().position(|t| {
                    l.panels
                        .expanded_threads
                        .contains(&format!("active:{}", t.id))
                });
                let next = if backwards {
                    current
                        .unwrap_or(0)
                        .checked_sub(1)
                        .unwrap_or(r.threads.len().saturating_sub(1))
                } else {
                    current.map_or(0, |i| (i + 1) % r.threads.len().max(1))
                };
                if let Some(id) = r.threads.get(next).map(|t| t.id.clone()) {
                    jump_thread(s, &id, tx);
                }
            })
            .build(k),
        );
    }
    let mut body = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(toolbar)
        .child(
            (if compact { col() } else { row() })
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(10.0)
                .children(show_nav.then_some(nav))
                .child(code.grow(1.0).min_w(0.0)),
        );
    if let (Some(page), Some(head)) = (review.next_page, review.head()) {
        body = body.child(
            action(
                "Load more files",
                Task::ReviewFiles {
                    project: project.into(),
                    url: url.into(),
                    head: head.into(),
                    page,
                },
                s,
                k,
            )
            .key("review-more-files"),
        );
    }
    body
}

fn jump_thread(s: &mut Workspace, id: &str, tx: &mut Tx) {
    let l = live_mut(s);
    let Some(thread) = l
        .panels
        .review
        .as_ref()
        .and_then(|key| l.app.services.reviews.get(key))
        .and_then(|r| r.threads.iter().find(|t| t.id == id))
    else {
        return;
    };
    l.panels.review_tab = 1;
    l.panels.review_restore = None;
    l.panels.review_file = Some(thread.path.clone());
    l.panels
        .expanded_threads
        .retain(|t| !t.starts_with("active:"));
    l.panels.expanded_threads.insert(format!("active:{id}"));
    l.panels.expanded_threads.insert(id.into());
    if let Some(i) = l.panels.diff.file_index(&thread.path) {
        if let Some(line) = thread.line.filter(|_| !thread.is_outdated) {
            l.panels.diff.scroll_to_line(
                i,
                diff::LineRef::new(
                    if thread.diff_side == "LEFT" {
                        diff::Side::Old
                    } else {
                        diff::Side::New
                    },
                    line as u32,
                ),
                tx,
            );
        } else {
            l.panels.diff.scroll_to_file(i, tx);
        }
    }
}

fn comment_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    comment: &caelum_protocol::GitHubReviewComment,
) -> El {
    let key = (project.into(), url.into());
    let id = comment.id.clone();
    let selected = live(s)
        .app
        .services
        .reviews
        .get(&key)
        .is_some_and(|r| r.selected_comments.contains(&id));
    let link = comment.url.clone();
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(10.0)
        .fill(if selected { k.c.hover } else { k.c.layer_01 })
        .border(
            1.0,
            if selected {
                k.alpha(k.c.text_accent, 0.3)
            } else {
                k.c.border_base
            },
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    rune_kit::Checkbox::new(selected)
                        .label(format!(
                            "{} · {}",
                            comment
                                .author
                                .as_ref()
                                .map(|a| a.login.as_str())
                                .unwrap_or("Unknown"),
                            comment.state.as_deref().unwrap_or("Comment")
                        ))
                        .on_toggle(move |s: &mut Workspace| {
                            let review = live_mut(s).app.services.reviews.get_mut(&key).unwrap();
                            if !review.selected_comments.remove(&id) {
                                review.selected_comments.insert(id.clone());
                            }
                        })
                        .build(k)
                        .key(format!("select-comment-{}", comment.id)),
                )
                .child(rune::el().grow(1.0))
                .child(ui::caption(
                    comment.created_at.split('T').next().unwrap_or_default(),
                    k,
                ))
                .child(
                    quiet_button(&ui::EXTERNAL, "Open comment on GitHub")
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&link);
                        })
                        .build(k)
                        .label("Open comment on GitHub"),
                ),
        )
        .child(
            Markdown::new(&comment.body)
                .selectable(true)
                .render(&theme::markdown(k)),
        )
}
fn thread_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    thread: &caelum_protocol::GitHubReviewThread,
) -> El {
    let open = !thread.is_resolved || live(s).panels.expanded_threads.contains(&thread.id);
    if !open {
        let id = thread.id.clone();
        return Button::new(format!(
            "Resolved · {}:{} · {} replies",
            thread.path,
            thread.line.or(thread.original_line).unwrap_or(0),
            thread.comments.len()
        ))
        .variant(ButtonVariant::GhostMuted)
        .icon(&glyphs::CARET_RIGHT)
        .on_press(move |s: &mut Workspace| {
            live_mut(s).panels.expanded_threads.insert(id.clone());
        })
        .build(k)
        .key(format!("thread-{}", thread.id));
    }
    let id = thread.id.clone();
    let mut content = col().w_full().gap(8.0).child(
        Button::new(format!(
            "{}:{} · {}{} · {}",
            thread.path,
            thread.line.or(thread.original_line).unwrap_or(0),
            if thread.is_resolved {
                "Resolved"
            } else {
                "Open"
            },
            if thread.is_outdated {
                " · Outdated"
            } else {
                ""
            },
            thread.diff_side
        ))
        .variant(ButtonVariant::GhostMuted)
        .on_press_tx(move |s: &mut Workspace, tx| jump_thread(s, &id, tx))
        .build(k)
        .key(format!("thread-location-{}", thread.id)),
    );
    if live(s).panels.review_tab == 0
        && let Some(hunk) = thread.comments.first().and_then(|c| c.diff_hunk.as_ref())
    {
        content = content.child(
            text(hunk)
                .font(k.ty.mono)
                .color(k.c.text_muted)
                .wrap()
                .selectable(true),
        );
    }
    for comment in &thread.comments {
        content = content.child(comment_view(s, k, project, url, comment));
    }
    if thread.is_resolved {
        let id = thread.id.clone();
        content = content.child(
            Button::new("Collapse resolved thread")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).panels.expanded_threads.remove(&id);
                })
                .build(k),
        );
    }
    if let Some(cursor) = &thread.next_cursor
        && let Some(head) = live(s)
            .app
            .services
            .reviews
            .get(&(project.into(), url.into()))
            .and_then(|r| r.head())
    {
        content = content.child(action(
            "More replies",
            Task::Feedback {
                project: project.into(),
                url: url.into(),
                head: head.into(),
                kind: GitHubFeedbackKind::Replies,
                cursor: Some(cursor.clone()),
                thread: Some(thread.id.clone()),
            },
            s,
            k,
        ));
    }
    content
}

pub fn composer_context(s: &Workspace, k: &Kit, index: usize) -> El {
    let session = &live(s).app.sessions[index];
    let mut context = col().w_full().gap(4.0).max_h(180.0).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if let Some(error) = &live(s).app.persistence_error {
        context = context.child(label(error, k));
    }
    for attachment in &session.attachments {
        let id = attachment.id.clone();
        context = context.child(
            Button::new(format!(
                "{}{} ×",
                attachment.label(),
                if attachment.truncated {
                    " (truncated)"
                } else {
                    ""
                }
            ))
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.sessions[index]
                    .attachments
                    .retain(|f| f.id != id)
            })
            .build(k),
        );
    }
    if !session.queue.is_empty() || !session.inbox.items.is_empty() {
        context = context.child(ui::caption("Queued messages", k));
    }
    for item in &session.inbox.items {
        let id = item.id.clone();
        let retry = id.clone();
        let mut entry = row()
            .w_full()
            .gap(4.0)
            .center_y()
            .child(
                label(
                    format!(
                        "{}{}",
                        if item.pending {
                            "Saving… "
                        } else {
                            "Queued: "
                        },
                        caelum_core::application::split_context(&item.text)
                            .0
                            .lines()
                            .next()
                            .unwrap_or_default()
                    ),
                    k,
                )
                .grow(1.0)
                .min_w(0.0),
            )
            .child(
                Button::new("Remove")
                    .variant(ButtonVariant::GhostMuted)
                    .disabled(item.pending)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.remove_queued(index, &id))
                    .build(k)
                    .key(format!("remove-queued-{}", item.id)),
            );
        if let Some(error) = &item.error {
            entry = entry.child(label(error, k)).child(
                Button::new("Retry")
                    .disabled(item.pending)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.retry_queued(index, &retry))
                    .build(k),
            );
        }
        context = context.child(entry);
    }
    if !session.queue.is_empty()
        && let Some(error) = &session.error
    {
        context = context.child(label(error, k).color(k.c.danger.fg));
    }
    for (position, queued) in session.queue.iter().enumerate() {
        context = context.child(
            row()
                .w_full()
                .center_y()
                .gap(4.0)
                .child(
                    label(
                        format!("Queued: {}", queued.text.lines().next().unwrap_or_default()),
                        k,
                    )
                    .truncate()
                    .grow(1.0)
                    .min_w(0.0),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_UP, "Move queued message earlier")
                        .disabled(position == 0)
                        .on_press(move |s: &mut Workspace| {
                            let queue = &mut live_mut(s).app.sessions[index].queue;
                            if position > 0 && position < queue.len() {
                                queue.swap(position, position - 1);
                            }
                        })
                        .build(k),
                )
                .child(
                    Button::new("Edit")
                        .variant(ButtonVariant::GhostMuted)
                        .disabled(
                            !live(s).drafts[index].text.is_empty()
                                || !session.attachments.is_empty()
                                || session.attaching > 0,
                        )
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            if position < live.app.sessions[index].queue.len()
                                && live.drafts[index].text.is_empty()
                                && live.app.sessions[index].attachments.is_empty()
                            {
                                let queued = live.app.sessions[index].queue.remove(position);
                                let (text, attachments) =
                                    caelum_core::application::split_context(&queued.text);
                                live.drafts[index] = TextEdit::new(text);
                                live.app.sessions[index].attachments = attachments;
                                live.app.sessions[index].options = queued.options;
                                live.app.options_changed(index);
                                live.drafts[index].focused = true;
                            }
                        })
                        .build(k),
                )
                .child(
                    Button::new("Remove")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(move |s: &mut Workspace| {
                            let queue = &mut live_mut(s).app.sessions[index].queue;
                            if position < queue.len() {
                                queue.remove(position);
                            }
                        })
                        .build(k),
                ),
        );
    }
    if session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Resume queued messages")
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut Workspace| live_mut(s).app.resume_queue(index))
                .build(k),
        );
    }
    if !session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Pause queue")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.sessions[index].queue_paused = true
                })
                .build(k),
        );
    }
    if session.attaching > 0 {
        context = context.child(label("Reading attachments…", k));
    }
    col()
        .w_full()
        .gap(4.0)
        .child(context)
        .child(crate::completion::view(s, k))
}

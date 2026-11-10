//! Rune adapter: editors and disclosure state stay here; application requests,
//! provider activity and history reconciliation live in caelum-core.

use std::{cell::Cell, collections::HashSet, rc::Rc};

use caelum_core::application::{EntryKind, Event, Runtime, Scope, Workspace as Application};
use caelum_protocol::ApprovalDecision;
use rune::{
    El, FileDialogOptions, ScrollPos, ScrollSpec, TextEdit, TextStyle, Tx, Waker, col, icon, row,
    text, text_input,
};
use rune_kit::shell::{RowStatus, SidebarHeader, SidebarRow};
use rune_kit::{ButtonVariant, Dialog, Kit, Markdown, Placement, ScrollBar, glyphs};

use crate::context::EditMenu;
use crate::select::{Choice, Select};
use crate::ui::Button;
use crate::{PLUS, SEND, STOP, Workspace, on_key, quiet_button, theme, ui};

pub struct Live {
    pub context: crate::context::State,
    pub checkouts: crate::checkouts::State,
    pub completion: crate::completion::Menu,
    pub commands: crate::commands::State,
    pub models: crate::models::Picker,
    pub media: crate::media::State,
    pub app: Application,
    pub drafts: Vec<TextEdit>,
    pub panels: crate::panels::Panels,
    pub forms: crate::questions::Forms,
    pub evaluation: bool,
    pub fixture: bool,
    evaluation_ticks: usize,
    runtime: Option<Runtime>,
    store_lock: Option<caelum_core::application::StoreLock>,
    pub(crate) expanded: HashSet<(usize, String)>,
    pub(crate) collapsed_patches: HashSet<(usize, String)>,
    visible_entries: HashSet<(usize, String)>,
    pub current_prompt: Option<String>,
    pub(crate) patches: std::collections::HashMap<(usize, String), crate::code_view::Document>,
    menu: Option<(&'static str, usize)>,
    pub session_menu: Option<usize>,
    delete_confirmation: Option<usize>,
    pub shell_panel: Option<crate::shells::Panel>,
    pub(crate) collapsed_shells: HashSet<usize>,
    pub(crate) expanded_queues: HashSet<usize>,
    pub focus_key: Option<String>,
    focus_observation: Cell<Option<(u64, bool)>>,
    pub archive_hovered: bool,
    reading: Vec<Rc<Cell<ScrollPos>>>,
    last_selected: Option<usize>,
    history_lengths: Vec<usize>,
    last_screen: crate::panels::Screen,
    last_completion: Option<(String, String)>,
    review_refresh: Option<((String, String), std::time::Instant)>,
    shell_refresh: Option<std::time::Instant>,
    wake: Option<Waker>,
}

impl Live {
    pub(crate) fn start(&mut self, store: caelum_core::application::Store, wake: Waker) {
        match store.lock() {
            Ok(lock) => self.store_lock = Some(lock),
            Err(error) => {
                self.app.persistence_error = Some(error);
                return;
            }
        }
        self.app.enable_store(store.0);
        self.wake = Some(wake.clone());
        self.media = crate::media::State::new(wake.clone());
        self.runtime = Some(Runtime::new(move || wake()));
    }

    pub fn new(project: Option<String>) -> Self {
        let mut app = Application::default();
        if let Some(project) = project {
            app.open_project(project);
        }
        Self {
            context: Default::default(),
            checkouts: Default::default(),
            completion: Default::default(),
            commands: Default::default(),
            models: Default::default(),
            media: Default::default(),
            app,
            drafts: Vec::new(),
            panels: Default::default(),
            forms: Default::default(),
            evaluation: false,
            fixture: false,
            evaluation_ticks: 0,
            runtime: None,
            store_lock: None,
            expanded: HashSet::new(),
            collapsed_patches: Default::default(),
            visible_entries: Default::default(),
            current_prompt: None,
            patches: Default::default(),
            menu: None,
            session_menu: None,
            delete_confirmation: None,
            shell_panel: None,
            collapsed_shells: Default::default(),
            expanded_queues: Default::default(),
            focus_key: None,
            focus_observation: Cell::new(None),
            archive_hovered: false,
            reading: Vec::new(),
            last_selected: None,
            history_lengths: Vec::new(),
            last_screen: crate::panels::Screen::Conversation,
            last_completion: None,
            review_refresh: None,
            shell_refresh: None,
            wake: None,
        }
    }

    pub(crate) fn sync_editors(&mut self) {
        while self.drafts.len() < self.app.sessions.len() {
            let session = &self.app.sessions[self.drafts.len()];
            self.drafts.push(TextEdit::new(&session.draft));
            self.commands.previous_drafts.push(session.draft.clone());
            let mut position = ScrollPos::default();
            if session.reading >= 0.0 {
                position.offset[1] = session.reading;
                position.max[1] = session.reading + 10.0;
            }
            self.reading.push(Rc::new(Cell::new(position)));
        }
    }

    pub fn new_session(&mut self) {
        self.schedule();
        let Some(project) = self.app.active_project.clone() else {
            self.panels.overlay = Some(crate::panels::Screen::Launcher);
            return;
        };
        let provider = self.app.preferences.default_provider;
        let index = self.app.project_draft(Scope { project, provider });
        self.sync_editors();
        self.drafts[index].focused = true;
        self.menu = None;
        self.panels.screen = crate::panels::Screen::Conversation;
    }

    pub fn focus(&mut self) {
        if let Some(index) = self.app.selected {
            self.drafts[index].focused = true;
        }
    }

    pub(crate) fn archive(&mut self, index: usize, archived: bool) {
        let selected = self.app.selected;
        self.app.archive(index, archived);
        // Local archive can create and select its replacement synchronously. Rune
        // rebuilds after the press, before poll gets a chance to create its editor.
        self.sync_editors();
        self.menu = None;
        self.panels.overlay = None;
        self.session_menu = None;
        if self.app.selected != selected || (!archived && !self.app.sessions[index].archived) {
            self.panels.archived = false;
            self.panels.screen = crate::panels::Screen::Conversation;
            self.focus();
        }
    }

    pub(crate) fn request_delete_session(&mut self, index: usize) {
        if self.app.sessions.get(index).is_some_and(|session| {
            session.archived && !session.forgotten && !archive_disabled(self, index)
        }) {
            self.menu = None;
            self.session_menu = None;
            self.delete_confirmation = Some(index);
        }
    }

    pub(crate) fn confirm_delete_session(&mut self, index: usize) {
        if self.delete_confirmation.take() == Some(index) {
            let selected = self.app.selected;
            self.app.delete_archived_session(index);
            self.sync_editors();
            if self.app.selected != selected {
                self.panels.archived = false;
                self.panels.screen = crate::panels::Screen::Conversation;
                self.focus();
            }
        }
    }

    pub(crate) fn cancel_delete_session(&mut self) {
        self.delete_confirmation = None;
        self.focus();
    }

    pub(crate) fn close_project(&mut self, project: &str) {
        self.panels.remember(&mut self.app);
        for (index, editor) in self.drafts.iter().enumerate() {
            self.app.sessions[index].draft.clone_from(&editor.text);
        }
        let selected = self.app.selected;
        let review = self.panels.screen == crate::panels::Screen::Review
            && self
                .panels
                .review
                .as_ref()
                .is_some_and(|(p, _)| self.app.group_path(p) == self.app.group_path(project));
        self.app.close_project(project);
        self.sync_editors();
        self.session_menu = None;
        self.menu = None;
        if selected != self.app.selected || review {
            self.panels.screen = crate::panels::Screen::Conversation;
            self.panels.overlay = None;
            self.panels.archived = false;
            self.models = Default::default();
            self.commands.side_owner = None;
        }
    }

    pub(crate) fn apply(&mut self, event: Event) {
        if let Some(panel) = &mut self.shell_panel {
            panel.sync(&self.app);
        }
        let selected = self.app.selected;
        let was_archived = selected.is_some_and(|i| self.app.sessions[i].archived);
        self.app.apply(event);
        self.sync_editors();
        for (index, session) in self.app.sessions.iter_mut().enumerate() {
            if let Some(caret) = session.completion_caret.take() {
                self.completion.pending = None;
                self.drafts[index].replace_text(&session.draft);
                self.drafts[index].set_caret(caret);
                self.drafts[index].focused = self.app.selected == Some(index);
            }
            if let Some(caret) = session.side_completion_caret.take()
                && self.commands.side_owner == Some(index)
            {
                self.completion.pending = None;
                self.commands.side_editor.replace_text(&session.side_draft);
                self.commands.side_editor.set_caret(caret);
            }
        }
        if self.app.selected != selected
            || (was_archived
                && self
                    .app
                    .selected
                    .is_some_and(|i| !self.app.sessions[i].archived))
        {
            if matches!(
                self.panels.overlay,
                Some(
                    crate::panels::Screen::Launcher
                        | crate::panels::Screen::Worktrees
                        | crate::panels::Screen::CreateWorktree
                )
            ) {
                self.panels.overlay = None;
            }
            self.panels.archived = false;
            self.panels.screen = crate::panels::Screen::Conversation;
            self.focus();
        }
    }

    pub fn working(&self) -> bool {
        self.app
            .selected
            .is_some_and(|index| self.app.sessions[index].working())
    }

    pub fn stop(&mut self) {
        self.schedule();
        if let Some(index) = self.app.selected {
            self.app.cancel(index);
        }
    }

    pub fn send(&mut self, tx: &mut Tx) {
        self.schedule();
        let Some(index) = self.app.selected else {
            return;
        };
        if self.app.send(index, self.drafts[index].text.clone()) {
            self.drafts[index].clear();
            self.drafts[index].focused = true;
            tx.scroll_to_bottom(format!("live-transcript-{index}"));
        }
    }

    pub(crate) fn tick_archive(&mut self, now: u64) -> bool {
        self.app.tick_archive_notice(
            now,
            self.archive_hovered
                || self
                    .focus_key
                    .as_ref()
                    .is_some_and(|key| key == "undo-archive" || key == "dismiss-archive"),
        )
    }

    pub fn poll(&mut self, tx: &mut Tx) -> bool {
        self.app.conversation_hidden = self.panels.screen != crate::panels::Screen::Conversation
            || self.panels.overlay.is_some();
        let mut changed = self.media.poll();
        if let Some(panel) = &mut self.shell_panel {
            panel.sync(&self.app);
        }
        changed |= self.tick_archive(caelum_core::application::presentation::now());
        if self.app.last_archived.is_some() {
            tx.wake_after(std::time::Duration::from_millis(100));
        }
        if self.evaluation {
            self.evaluation_ticks += 1;
            if self.evaluation_ticks.is_multiple_of(3) {
                crate::evaluation::tick(&mut self.app);
                changed = true;
            }
            tx.wake_after(std::time::Duration::from_millis(16));
        }
        for (i, editor) in self.drafts.iter().enumerate() {
            self.app.sessions[i].draft.clone_from(&editor.text);
            let position = self.reading[i].get();
            self.app.sessions[i].reading = if position.offset[1] >= position.max[1] - 1.0 {
                -1.0
            } else {
                position.offset[1]
            };
        }
        if let Some(index) = self.commands.side_owner {
            if self.app.selected != Some(index) {
                self.app.dismiss_side_question(index);
                self.commands.side_owner = None;
            } else {
                self.app.sessions[index]
                    .side_draft
                    .clone_from(&self.commands.side_editor.text);
            }
        }
        if let Some((index, side, text, anchor, focus)) = &self.completion.pending {
            let editor = if *side {
                &self.commands.side_editor
            } else {
                &self.drafts[*index]
            };
            if self.app.selected != Some(*index)
                || !editor.focused
                || self.panels.overlay.is_some()
                || self.models.is_open()
                || editor.text != *text
                || editor.anchor != *anchor
                || editor.focus != *focus
            {
                self.app.cancel_mentions(*index);
                self.completion.pending = None;
            }
        }
        if let Some(index) = self.app.selected {
            let completion = self
                .commands
                .side_owner
                .map(|_| &self.commands.side_editor)
                .or_else(|| self.drafts.get(index))
                .filter(|draft| {
                    draft.focused
                        && draft.preedit.is_none()
                        && self.panels.overlay.is_none()
                        && !self.models.is_open()
                })
                .and_then(|draft| {
                    caelum_core::application::Completion::at(&draft.text, draft.anchor, draft.focus)
                })
                .filter(|token| token.kind == caelum_core::application::CompletionKind::File)
                .map(|token| (self.app.sessions[index].scope.project.clone(), token.query));
            if completion != self.last_completion {
                self.last_completion = completion.clone();
                if let Some((project, query)) = completion {
                    self.app
                        .task(caelum_core::application::Task::Completions { project, query });
                }
            }
        }
        let events = self.runtime.as_ref().map_or_else(Vec::new, Runtime::drain);
        changed |= !events.is_empty();
        for event in events {
            self.apply(event);
        }
        changed |= self.app.acknowledge();
        for (index, id) in &self.visible_entries {
            if self.collapsed_patches.contains(&(*index, id.clone())) {
                continue;
            }
            if let Some(entry) = self.app.sessions[*index]
                .conversation
                .entries
                .iter()
                .find(|e| &e.id == id)
            {
                let tool = caelum_core::application::presentation::Tool::from_entry(entry, 0);
                if !tool.patch.is_empty() {
                    changed |= self.patches.entry((*index, id.clone())).or_default().patch(
                        tool.paths.first().map(String::as_str).unwrap_or("Patch"),
                        &tool.patch,
                    );
                }
            }
        }
        if self.patches.len() > 48 {
            self.patches
                .retain(|key, _| self.visible_entries.contains(key));
        }
        if self.runtime.is_some() {
            changed |= self.checkouts.sync(&mut self.app);
            self.runtime
                .as_mut()
                .unwrap()
                .watch_checkouts(self.app.projects.iter().map(|p| p.path.clone()).collect());
            if let Some(index) = self
                .shell_panel
                .as_ref()
                .filter(|_| self.panels.overlay == Some(crate::panels::Screen::Shells))
                .map(|p| p.index)
                .or(self.app.selected)
                && !self.app.sessions[index].shells.is_empty()
                && self
                    .shell_refresh
                    .is_none_or(|at| at.elapsed().as_secs() >= 2)
            {
                self.shell_refresh = Some(std::time::Instant::now());
                let session = &self.app.sessions[index];
                let tasks: Vec<_> = session
                    .shells
                    .iter()
                    .filter_map(|shell| {
                        Some(caelum_core::application::Task::Shell {
                            project: session.scope.project.clone(),
                            session: session.remote_id.clone()?,
                            id: shell.id.clone(),
                            stop: false,
                        })
                    })
                    .collect();
                for task in tasks {
                    self.app.task(task);
                }
            }
            if self.panels.screen == crate::panels::Screen::Review
                && let Some(key) = self.panels.review.clone()
            {
                let now = std::time::Instant::now();
                match &self.review_refresh {
                    Some((previous, at))
                        if previous == &key && now.duration_since(*at).as_secs() >= 30 =>
                    {
                        self.app.task(caelum_core::application::Task::Details {
                            project: key.0.clone(),
                            url: key.1.clone(),
                        });
                        self.review_refresh = Some((key, now));
                    }
                    Some((previous, _)) if previous == &key => {}
                    _ => self.review_refresh = Some((key, now)),
                }
                tx.wake_after(std::time::Duration::from_secs(30));
            } else {
                self.review_refresh = None;
            }
            for (id, command) in self.app.commands() {
                if let Err(error) = self.runtime.as_mut().unwrap().submit(id, command) {
                    self.apply(Event::Completed(id, Err(error)));
                    changed = true;
                }
            }
        }
        self.sync_editors();
        self.forms.sync(&self.app.questions);
        self.panels.remember(&mut self.app);
        changed |= self.panels.sync(&self.app);
        if let Some((key, review)) = self
            .panels
            .restoring_review()
            .and_then(|key| self.app.services.reviews.get(key).map(|r| (key.clone(), r)))
            && review.files_loaded
            && let (Some(page), Some(head)) = (review.next_page, review.head())
        {
            self.app.task(caelum_core::application::Task::ReviewFiles {
                project: key.0,
                url: key.1,
                head: head.into(),
                page,
            });
        }
        self.panels.reveal_line(tx);
        if let Some(index) = self.models.reveal.take() {
            tx.scroll_center_in_instant("model-list", format!("option{index}"));
        }
        self.history_lengths.resize(self.app.sessions.len(), 0);
        for (index, session) in self.app.sessions.iter().enumerate() {
            let length = session.conversation.entries.len();
            if length != self.history_lengths[index]
                && !session.working()
                && self.app.selected == Some(index)
            {
                // Restored history is already complete; opening it must not
                // reveal thousands of pixels at the live-stream growth speed.
                tx.instant();
            }
            self.history_lengths[index] = length;
        }
        if self.last_selected != self.app.selected
            || (self.panels.screen == crate::panels::Screen::Conversation
                && self.last_screen != self.panels.screen)
        {
            if self.last_selected.is_some()
                && self.last_selected != self.app.selected
                && let Some(project) = &self.app.active_project
                && self.app.group_path(project) != project
            {
                let root = self.app.group_path(project).to_owned();
                self.app
                    .preferences
                    .worktree_expanded
                    .entry(root)
                    .or_insert(true);
            }
            self.menu = None;
            if let Some(previous) = self.last_selected
                && self.last_selected != self.app.selected
            {
                self.app.sessions[previous].completion_generation += 1;
            }
            self.last_selected = self.app.selected;
            if let Some(index) = self.app.selected {
                let position = self.reading[index].get();
                let key = format!("live-transcript-{index}");
                if position.offset[1] >= position.max[1] - 1.0 {
                    tx.scroll_to_bottom_instant(key);
                } else {
                    tx.scroll_to_y(key, position.offset[1]);
                }
            }
            changed = true;
        }
        self.last_screen = self.panels.screen;
        if self
            .app
            .sessions
            .iter()
            .any(|s| s.working() || !s.shells.is_empty())
        {
            tx.wake_after(std::time::Duration::from_secs(1));
        }
        changed
    }

    fn schedule(&self) {
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.panels.remember(&mut self.app);
        for (i, editor) in self.drafts.iter().enumerate() {
            self.app.sessions[i].draft.clone_from(&editor.text);
        }
        drop(self.runtime.take());
        if let Err(error) = self.app.flush() {
            eprintln!("{error}");
        }
    }
}

pub fn setup(s: &mut Workspace, wake: Waker) {
    if let Some(live) = &mut s.live {
        if live.evaluation || live.fixture {
            return;
        }
        live.start(
            caelum_core::application::Store(caelum_core::application::Store::default_path()),
            wake,
        );
    }
}

pub(crate) fn live(s: &Workspace) -> &Live {
    s.live.as_ref().expect("live workspace")
}
pub(crate) fn live_mut(s: &mut Workspace) -> &mut Live {
    let live = s.live.as_mut().expect("live workspace");
    // Rune polls before input handlers. Mutations must schedule the following
    // poll so commands and derived views also advance in an otherwise idle app.
    live.schedule();
    live
}

pub fn open_project(_: &mut Workspace, tx: &mut Tx) {
    tx.open_folder(
        FileDialogOptions {
            title: "Open a project".into(),
            ..Default::default()
        },
        |s: &mut Workspace, result, _| match result {
            Ok(Some(path)) => {
                let l = live_mut(s);
                if l.panels.overlay == Some(crate::panels::Screen::Launcher) {
                    l.app.launch_project(
                        path.to_string_lossy().into_owned(),
                        l.panels.launch_provider,
                    );
                } else {
                    l.app.open_project(path.to_string_lossy().into_owned());
                }
            }
            Ok(None) => {}
            Err(error) => live_mut(s).app.error = Some(error.to_string()),
        },
    );
}

pub fn sidebar(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let roots: Vec<_> = app
        .projects
        .iter()
        .filter(|project| app.group_path(&project.path) == project.path)
        .collect();
    let mut sidebar = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .label("Projects and sessions")
        .gap(4.0)
        .child(ui::brand(s, k))
        .child(
            SidebarHeader::new(if app.opening {
                "Opening project…"
            } else {
                "Projects"
            })
            .count(roots.len())
            .action(
                quiet_button(&PLUS, "Open project")
                    .on_press_tx(open_project)
                    .build(k)
                    .key("open-project"),
            )
            .build(k)
            .key("projects-heading"),
        );
    let mut projects = col()
        .key("projects-list")
        .role("list")
        .w_full()
        .gap(4.0)
        .focus_group();
    let mut projects_height = (roots.len() as f32 * 74.0 - 4.0).max(40.0);
    let mut expanded_worktrees = false;
    for project in &roots {
        let path = project.path.clone();
        let activity = app
            .sessions
            .iter()
            .filter(|session| app.group_path(&session.scope.project) == path && session.working())
            .count();
        let requests = app
            .approvals
            .iter()
            .filter(|p| app.group_path(&p.project_path) == path)
            .count()
            + app
                .questions
                .iter()
                .filter(|p| app.group_path(&p.project_path) == path)
                .count();
        let selected = app
            .active_project
            .as_ref()
            .is_some_and(|active| app.group_path(active) == path);
        let active = app
            .selected
            .map(|i| &app.sessions[i])
            .filter(|s| app.group_path(&s.scope.project) == path)
            .or_else(|| {
                app.sessions.iter().find(|s| {
                    app.group_path(&s.scope.project) == path && !s.archived && !s.forgotten
                })
            });
        let checkout = &path;
        let git = app
            .services
            .checkouts
            .get(checkout)
            .and_then(|c| c.git.as_ref());
        let branch = git
            .and_then(|g| g.branch.as_deref())
            .or_else(|| {
                app.services
                    .worktrees(&path)?
                    .worktrees
                    .iter()
                    .find(|tree| &tree.project.path == checkout)?
                    .branch
                    .as_deref()
            })
            .unwrap_or("No branch");
        let detail = if requests > 0 {
            format!("{requests} need attention")
        } else if activity > 0 {
            format!("{activity} working")
        } else {
            git.map(|g| format!("{} changed", g.changes.len()))
                .unwrap_or_default()
        };
        let context_path = path.clone();
        let close_path = path.clone();
        let project_key = crate::code_view::source_key(&path, "project");
        let worktree_project = path.clone();
        let worktree_provider = active
            .map(|s| s.scope.provider)
            .unwrap_or(app.preferences.default_provider);
        let mut group = col().w_full()
            .key(crate::code_view::source_key(&project.path, "project-group"))
            .role("group").label(format!("{} project and worktrees", project.name))
            .child(crate::context::project(
            k.interactive(
                col()
                    .w_full()
                    .h(70.0)
                    .gap(5.0)
                    .padding(9.0)
                    .radius(10.0)
                    .fill(rune::Color::TRANSPARENT)
                    .child(
                        row()
                            .gap(7.0)
                            .center_y()
                            .child(icon(&ui::FOLDER, 14.0).color(if selected { k.c.icon_accent } else { k.c.icon_muted }))
                            .child(
                                text(&project.name)
                                    .font(k.ty.ui_medium)
                                    .color(k.c.text_base)
                                    .truncate()
                                    .min_w(0.0)
                                    .grow(1.0),
                            )
                            .child(
                                quiet_button(&ui::BRANCH, "Create worktree")
                                    .on_press(move |s: &mut Workspace| {
                                        crate::worktree::open(
                                            s,
                                            worktree_project.clone(),
                                            worktree_provider,
                                        )
                                    })
                                    .build(k)
                                    .size(22.0, 24.0)
                                    .key(crate::code_view::source_key(&path, "create-worktree")),
                            )
                            .child(
                                quiet_button(&glyphs::CLOSE, "Close project")
                                    .on_press(move |s: &mut Workspace| live_mut(s).close_project(&close_path))
                                    .build(k)
                                    .key(crate::code_view::source_key(&path, "close-project"))
                                    .label(format!("Close {}", project.name))
                                    .help("Close project · Reopen it to return to your sessions and drafts")
                                    .size(22.0, 24.0),
                            ),
                    )
                    .child(
                        row()
                            .gap(5.0)
                            .center_y()
                            .pl(21.0)
                            .child(icon(&ui::BRANCH, 11.0).color(k.c.icon_muted))
                            .child(
                                text(format!("Project folder · {branch}"))
                                    .font(k.ty.small)
                                    .color(k.c.text_muted)
                                    .truncate()
                                    .min_w(0.0)
                                    .grow(1.0),
                            )
                            .child(text(detail).font(rune::TextStyle::new(10.0, 400.0)).color(
                                if requests > 0 {
                                    k.c.warning.icon
                                } else {
                                    k.c.text_muted
                                },
                            )),
                    ),
            )
            .role("button")
            .label(&project.name)
            .description(&path)
            .help(&path)
            .focus_group()
            .focus_return(if roots.len() == 1 { "open-project" } else { "projects-list" })
            .focusable(true)
            .focus_ring(1.0, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                crate::checkouts::select(s, &path);
            })
            .key(project_key),
            context_path,
        ));
        if let Some((worktrees, height)) = crate::checkouts::sidebar(s, k, &project.path) {
            group = group.child(worktrees);
            projects_height += height + 4.0;
            expanded_worktrees |= height > 36.0;
        }
        projects = projects.child(group);
    }
    sidebar = sidebar
        .child(
            col()
                .key("projects-scroll")
                .w_full()
                .grow(if expanded_worktrees { 2.5 } else { 1.2 })
                .min_h(0.0)
                .max_h(projects_height)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(if roots.is_empty() {
                    ui::caption("Open a project to get started", k).padding(8.0)
                } else {
                    projects
                })
                .scroll_bar(k),
        )
        .child(
            row()
                .w_full()
                .gap(4.0)
                .center_y()
                .mt(4.0)
                .mb(4.0)
                .child(
                    Button::new("New session")
                        .icon(&PLUS)
                        .variant(ButtonVariant::Outline)
                        .on_press(|s: &mut Workspace| s.new_session())
                        .build(k)
                        .key("new-session")
                        .h(32.0)
                        .grow(1.0),
                )
                .child(crate::panels::navigation(s, k)),
        )
        .child(
            SidebarHeader::new(if live(s).panels.archived {
                "Archived sessions"
            } else {
                "Sessions"
            })
            .count(
                app.sessions
                    .iter()
                    .filter(|session| {
                        !session.forgotten
                            && session.archived == live(s).panels.archived
                            && app.active_project.as_ref().is_some_and(|p| {
                                app.group_path(&session.scope.project) == app.group_path(p)
                            })
                    })
                    .count(),
            )
            .action(
                row()
                    .gap(1.0)
                    .child(
                        quiet_button(
                            &ui::ARCHIVE,
                            if live(s).panels.archived {
                                "Show active sessions"
                            } else {
                                "Show archived sessions"
                            },
                        )
                        .expanded(live(s).panels.archived)
                        .on_press(|s: &mut Workspace| {
                            let p = &mut live_mut(s).panels;
                            p.archived = !p.archived;
                        })
                        .build(k)
                        .key("toggle-archived"),
                    )
                    .child(
                        quiet_button(&glyphs::SEARCH, "Search sessions")
                            .on_press(|s: &mut Workspace| {
                                let panels = &mut live_mut(s).panels;
                                panels.searching_sessions = !panels.searching_sessions;
                                panels.sessions.focused = panels.searching_sessions;
                            })
                            .build(k)
                            .key("search-sessions"),
                    )
                    .child(
                        quiet_button(&ui::REFRESH, "Refresh sessions")
                            .disabled(
                                app.active_project.is_none()
                                    && !app.sessions.iter().any(|session| {
                                        !session.archived
                                            && !session.forgotten
                                            && session.remote_id.is_some()
                                            && app.project_is_open(&session.scope.project)
                                    }),
                            )
                            .on_press(|s: &mut Workspace| {
                                let app = &mut live_mut(s).app;
                                app.refresh_project();
                                app.refresh_open_sessions();
                            })
                            .build(k)
                            .key("refresh-sessions"),
                    ),
            )
            .build(k)
            .key("sessions-heading"),
        );
    if live(s).panels.searching_sessions {
        sidebar = sidebar.child(
            text_input(&live(s).panels.sessions, k.ty.ui)
                .label("Search sessions")
                .placeholder("Search sessions · ⌘K", k.c.text_muted)
                .color(k.c.text_base)
                .w_full()
                .padding(6.0)
                .radius(6.0)
                .border(1.0, k.c.border_base)
                .edit_menu("session-search", |s| Some(&mut live_mut(s).panels.sessions)),
        );
    }
    let mut sessions = col()
        .w_full()
        .gap(3.0)
        .key("sidebar-list")
        .role("list")
        .focus_group()
        .focus_follows();
    for (index, session) in
        app.sessions.iter().enumerate().filter(|(_, session)| {
            !session.forgotten
                && session.archived == live(s).panels.archived
                && app.project_is_open(&session.scope.project)
                && (app.active_project.as_ref().is_some_and(|path| {
                    app.group_path(&session.scope.project) == app.group_path(path)
                }) || !live(s).panels.sessions.text.is_empty())
                && format!(
                    "{} {} {}",
                    session.title,
                    session.scope.provider.as_str(),
                    session.scope.project
                )
                .to_lowercase()
                .contains(&live(s).panels.sessions.text.to_lowercase())
        })
    {
        let pending = app.approvals.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && p.session_id == session.remote_id
        }) || app.questions.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && Some(&p.session_id) == session.remote_id.as_ref()
        });
        let branch = session
            .details
            .as_ref()
            .and_then(|d| d.branch.as_deref())
            .or_else(|| {
                app.services
                    .checkouts
                    .get(&session.scope.project)
                    .and_then(|c| c.git.as_ref())
                    .and_then(|g| g.branch.as_deref())
            })
            .or_else(|| {
                app.services
                    .worktrees(&session.scope.project)?
                    .worktrees
                    .iter()
                    .find(|tree| tree.project.path == session.scope.project)?
                    .branch
                    .as_deref()
            })
            .unwrap_or(session.scope.provider.as_str());
        let detail = format!(
            "⑂ {} · {}",
            branch,
            if pending {
                "Needs an answer"
            } else if session.archiving {
                "Archiving…"
            } else if session.deleting {
                "Deleting…"
            } else if session.stopping {
                "Stopping…"
            } else if session.working() {
                "Working…"
            } else if session.loading {
                "Loading history…"
            } else if let Some(error) = session.error.as_deref() {
                error
            } else {
                session.scope.project.rsplit('/').next().unwrap_or_default()
            }
        );
        let mut item = ui::session_row(&session.title, &detail, app.selected == Some(index), k)
            .on_select(move |s: &mut Workspace, _| {
                let live = live_mut(s);
                let worktree_group = live
                    .app
                    .group_path(&live.app.sessions[index].scope.project)
                    .to_owned();
                live.app.select(index);
                live.app
                    .preferences
                    .worktree_expanded
                    .entry(worktree_group)
                    .or_insert(false);
                live.panels.screen = crate::panels::Screen::Conversation;
                live.menu = None;
            });
        if pending {
            item = item.status(RowStatus::Attention);
        } else if session.working() {
            let thinking = crate::activity::session_activity(s, index).is_some_and(|a| {
                a.kind == caelum_core::application::presentation::ActivityKind::Thinking
            });
            item = item.status(RowStatus::Other(ui::busy_icon(
                if thinking { &ui::BRAIN } else { &ui::CLOCK },
                12.0,
                k,
            )));
        } else if session.unread {
            item = item.status(RowStatus::Unread);
        }
        {
            item = item.trailing(
                quiet_button(&ui::MORE, "Session actions")
                    .on_press(move |s: &mut Workspace| {
                        live_mut(s).session_menu = Some(index);
                    })
                    .build(k)
                    .group_opacity(
                        if app.selected == Some(index)
                            || live(s).focus_key.as_ref().is_some_and(|key| {
                                key.ends_with(&format!("session-actions-{index}"))
                            })
                        {
                            1.0
                        } else {
                            0.0
                        },
                        1.0,
                    )
                    .size(22.0, 24.0)
                    .key(format!("session-actions-{index}")),
            );
        }
        if !pending
            && !session.working()
            && !session.archiving
            && !session.deleting
            && session.queue.is_empty()
            && session.inbox.items.is_empty()
        {
            item = item.on_close(move |s: &mut Workspace, _| {
                let live = live_mut(s);
                if live.app.sessions[index].archived {
                    live.request_delete_session(index);
                } else {
                    live.archive(index, true);
                }
            });
        }
        sessions = sessions.child(ui::selected_row(
            // Row actions must not inherit the list's selection-following focus.
            crate::context::session(
                item.build(k)
                    .focus_group()
                    .key(format!("live-session-{index}")),
                index,
            ),
            app.selected == Some(index),
            k,
        ));
    }
    sidebar = sidebar.child(
        col()
            .key("sidebar-scroll")
            .w_full()
            .grow(1.0)
            .min_h(40.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(sessions)
            .scroll_bar(k),
    );
    let mut seen = HashSet::new();
    let mut requests = col()
        .key("requests-list")
        .role("list")
        .w_full()
        .gap(4.0)
        .focus_group();
    for (project, checkout) in &app.services.checkouts {
        if app
            .active_project
            .as_ref()
            .is_none_or(|p| app.group_path(project) != app.group_path(p))
        {
            continue;
        }
        for pr in &checkout.pull_requests {
            if !seen.insert(&pr.url) {
                continue;
            }
            let project = project.clone();
            let url = pr.url.clone();
            let context_project = project.clone();
            requests = requests.child(crate::context::pull_request(
                SidebarRow::new(&pr.title)
                    .icon(&ui::BRANCH)
                    .subtitle(format!("#{} · into {}", pr.number, pr.base_ref_name))
                    .height(52.0)
                    .title_style(k.ty.ui)
                    .subtitle_style(k.ty.small)
                    .on_select(move |s: &mut Workspace, _| {
                        crate::panels::open_review(s, project.clone(), url.clone())
                    })
                    .build(k)
                    .help(format!("#{} · {}", pr.number, pr.title))
                    .key(format!("pull-request-{}", pr.number)),
                context_project,
                pr.url.clone(),
                pr.number,
            ));
        }
    }
    sidebar = sidebar
        .child(rune::el().w_full().h(1.0).fill(k.c.border_base))
        .child(
            SidebarHeader::new("Pull requests")
                .count(seen.len())
                .build(k)
                .key("requests-heading"),
        )
        .child(
            col()
                .key("requests-scroll")
                .w_full()
                .grow(1.2)
                .max_h(if seen.is_empty() {
                    44.0
                } else {
                    seen.len() as f32 * 56.0 - 4.0
                })
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(if seen.is_empty() {
                    if app.services.checkouts.iter().any(|(project, checkout)| {
                        checkout
                            .git
                            .as_ref()
                            .and_then(|g| g.branch.as_ref())
                            .is_some_and(|branch| {
                                app.services
                                    .busy(&caelum_core::application::Task::Discover {
                                        project: project.clone(),
                                        branch: branch.clone(),
                                    })
                            })
                    }) {
                        ui::skeleton("Loading pull requests", 2, k).px(8.0)
                    } else {
                        ui::caption("No pull requests for open sessions", k).padding(8.0)
                    }
                } else {
                    requests
                })
                .scroll_bar(k),
        );
    sidebar
}

pub fn transcript(s: &Workspace, k: &Kit, reduced: bool, padding: f32, layers: &mut Vec<El>) -> El {
    if live(s).panels.screen == crate::panels::Screen::Review {
        return crate::panels::view(s, k, layers);
    }
    let live = live(s);
    let mut content = col().w_full().max_w(840.0).px(padding).py(24.0).gap(24.0);
    if let Some(error) = &live.app.persistence_error {
        content = content.child(text(error).font(k.ty.ui).color(k.c.text_accent).wrap());
    }
    if let Some(error) = &live.app.error {
        content = content.child(
            text(error.clone())
                .font(k.ty.ui)
                .color(k.c.text_accent)
                .wrap()
                .selectable(true),
        );
    }
    let Some(index) = live.app.selected else {
        return content
            .child(ui::empty(
                "A little room to think",
                "Open a project to work with Codex or OpenCode.",
                k,
            ))
            .child(
                Button::new("Open project")
                    .variant(ButtonVariant::Outline)
                    .on_press_tx(open_project)
                    .build(k)
                    .key("welcome-open"),
            )
            .grow(1.0);
    };
    let visible: Rc<Vec<_>> = Rc::new(
        live.app.sessions[index]
            .conversation
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| caelum_core::application::presentation::entry_visible(e))
            .map(|(i, _)| i)
            .collect(),
    );
    let count = visible.len();
    let keys = visible.clone();
    let rows = visible.clone();
    let reading = live.reading[index].clone();
    let kit = k.clone();
    col()
        .key(format!("live-transcript-{index}"))
        .role("region")
        .label("Conversation")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .scroll(ScrollSpec { stick_bottom: true })
        .on_scroll_offset(move |position| reading.set(position))
        .child(
            col()
                .key(format!("live-rows-{index}"))
                .w_full()
                .max_w(840.0)
                .growth(rune::GrowthSpec::default())
                .virtual_rows(
                    count + 2,
                    130.0,
                    move |s: &Workspace, row| {
                        if row == 0 {
                            return format!("live-header-{index}").into();
                        }
                        match crate::live::live(s).app.sessions[index]
                            .conversation
                            .entries
                            .get(keys.get(row - 1).copied().unwrap_or(usize::MAX))
                        {
                            Some(entry) => format!("live-entry-{index}-{}", entry.id).into(),
                            None => format!("live-tail-{index}").into(),
                        }
                    },
                    move |s: &Workspace, row| {
                        timeline_row(
                            s,
                            &kit,
                            reduced,
                            padding,
                            index,
                            if row == 0 {
                                0
                            } else {
                                rows.get(row - 1).map_or(usize::MAX, |i| i + 1)
                            },
                        )
                    },
                )
                .on_visible_rows(move |s: &mut Workspace, range| {
                    let positions: Vec<_> = range
                        .filter_map(|row| row.checked_sub(1).and_then(|i| visible.get(i).copied()))
                        .collect();
                    let l = live_mut(s);
                    let entries = &l.app.sessions[index].conversation.entries;
                    let next: HashSet<_> = positions
                        .iter()
                        .filter_map(|i| entries.get(*i).map(|e| (index, e.id.clone())))
                        .collect();
                    let changed = next != l.visible_entries;
                    l.visible_entries = next;
                    if let Some(first) = positions.first().filter(|i| **i < entries.len()) {
                        l.current_prompt = entries[..=*first]
                            .iter()
                            .rev()
                            .find(|e| {
                                e.kind == EntryKind::User
                                    && caelum_core::application::presentation::entry_visible(e)
                            })
                            .map(|e| e.id.clone());
                    }
                    crate::media::request(s, index, &positions);
                    changed
                }),
        )
}

fn timeline_row(
    s: &Workspace,
    k: &Kit,
    reduced: bool,
    padding: f32,
    index: usize,
    row_index: usize,
) -> El {
    let live = live(s);
    let mut content = col().w_full().px(padding).py(12.0).gap(16.0);
    let session = &live.app.sessions[index];
    if row_index == 0 {
        if let Some(error) = &live.app.error {
            content = content.child(text(error).font(k.ty.ui).color(k.c.text_accent).wrap());
        }
        if session.loading {
            content = content.child(ui::skeleton("Loading history", 3, k));
        }
        if session.next_cursor.is_some() {
            content = content.child(
                Button::new("Load older messages")
                    .variant(ButtonVariant::GhostMuted)
                    .disabled(session.loading)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.older(index))
                    .build(k)
                    .key("load-older")
                    .self_start(),
            );
        }
        let needs_input = live.app.approvals.iter().any(|p| {
            p.provider == session.scope.provider.as_str()
                && p.project_path == session.scope.project
                && p.session_id == session.remote_id
        }) || live.app.questions.iter().any(|p| {
            p.provider == session.scope.provider.as_str()
                && p.project_path == session.scope.project
                && Some(&p.session_id) == session.remote_id.as_ref()
        });
        if session.conversation.entries.is_empty() && !session.loading && !needs_input {
            content = content.child(ui::empty(
                "A little room to think",
                "Ask a question, describe a change, or @mention a file.",
                k,
            ));
            if let Some(error) = live.app.notices.get(&session.scope) {
                content = content.child(
                    ui::notice(
                        &format!("Connect {}", ui::provider_name(session.scope.provider)),
                        error,
                        k,
                    )
                    .child(
                        Button::new("Provider settings")
                            .variant(ButtonVariant::GhostMuted)
                            .on_press(move |s: &mut Workspace| {
                                let l = live_mut(s);
                                l.panels.settings_page = crate::settings::Page::Providers;
                                l.panels.settings_provider = l.app.sessions[index].scope.provider;
                                crate::panels::show(s, crate::panels::Screen::Settings);
                            })
                            .build(k)
                            .self_start(),
                    ),
                );
            }
        }
        return content;
    }
    if let Some(entry) = session.conversation.entries.get(row_index - 1) {
        let mut item = col()
            .w_full()
            .gap(8.0)
            .key(format!("live-entry-{index}-{}", entry.id));
        match entry.kind {
            EntryKind::User => {
                let (_, files) = caelum_core::application::split_context(&entry.text);
                let message = caelum_core::application::presentation::entry_text(entry);
                let mut bubble =
                    ui::user_bubble(&message, !files.is_empty() || !entry.media.is_empty(), k)
                        .key(format!("user-bubble-{index}-{}", entry.id));
                if !message.trim().is_empty() {
                    bubble = bubble.child(
                        Markdown::new(&message)
                            .selectable(true)
                            .on_link_tx(move |s: &mut Workspace, url, tx| {
                                crate::panels::open_link(s, index, url, tx)
                            })
                            .on_link_secondary(move |s: &mut Workspace, url, at, tx| {
                                crate::context::link(s, index, url, at, tx)
                            })
                            .render(&ui::user_markdown(k)),
                    );
                }
                let mut attachments = col().w_full().gap(6.0);
                let has_files = !files.is_empty() || !entry.media.is_empty();
                for file in files {
                    if file.kind == caelum_core::application::AttachmentKind::Image {
                        attachments = attachments.child(crate::media::thumbnail(
                            s,
                            k,
                            &session.scope.project,
                            &caelum_core::application::presentation::Media {
                                name: file.path,
                                source: file.content,
                            },
                        ));
                        continue;
                    }
                    let path = file.path.clone();
                    attachments = attachments.child(
                        k.interactive(
                            row()
                                .w_full()
                                .max_w(600.0)
                                .gap(7.0)
                                .px(8.0)
                                .py(5.0)
                                .radius(6.0)
                                .fill(k.c.layer_01)
                                .child(icon(&ui::FILE, 13.0).color(k.c.icon_muted))
                                .child(
                                    text(format!(
                                        "{}{}",
                                        file.label(),
                                        if file.truncated { " · truncated" } else { "" }
                                    ))
                                    .font(k.ty.small)
                                    .color(k.c.text_muted)
                                    .truncate()
                                    .min_w(0.0),
                                ),
                        )
                        .role("button")
                        .label(file.label())
                        .focusable(true)
                        .focus_ring(1.0, 0.0, k.c.border_focus)
                        .on_press_tx(move |s: &mut Workspace, tx| {
                            crate::panels::open_link(s, index, &path, tx)
                        }),
                    );
                }
                for media in &entry.media {
                    attachments = attachments.child(crate::media::thumbnail(
                        s,
                        k,
                        &session.scope.project,
                        media,
                    ));
                }
                if has_files {
                    bubble = bubble.child(attachments);
                }
                let copy_key = format!("copy-user-{index}-{}", entry.id);
                let focused = live
                    .focus_key
                    .as_ref()
                    .is_some_and(|key| key.ends_with(&copy_key));
                let mut turn = row().w_full().justify_end().gap(6.0);
                if !message.is_empty() {
                    turn = turn.child(
                        quiet_button(&glyphs::COPY, "Copy message")
                            .on_press_tx(move |_: &mut Workspace, tx| tx.copy(message.clone()))
                            .build(k)
                            .key(copy_key)
                            .self_end()
                            .group_opacity(if focused { 1.0 } else { 0.0 }, 1.0),
                    );
                }
                item = item.hover_group().child(turn.child(bubble));
            }
            EntryKind::Assistant => {
                let id = entry.id.clone();
                item = item
                    .child(
                        row()
                            .w_full()
                            .center_y()
                            .justify_between()
                            .child(
                                row()
                                    .center_y()
                                    .gap(7.0)
                                    .child(icon(&glyphs::SPARKLE, 13.0).color(k.c.icon_muted))
                                    .child(
                                        text(if entry.complete {
                                            ui::provider_name(session.scope.provider)
                                        } else {
                                            "Responding…"
                                        })
                                        .font(k.ty.small)
                                        .color(k.c.text_muted),
                                    ),
                            )
                            .child(
                                quiet_button(&glyphs::COPY, "Copy response")
                                    .on_press_tx(move |s: &mut Workspace, tx| {
                                        if let Some(entry) = live_mut(s).app.sessions[index]
                                            .conversation
                                            .entries
                                            .iter()
                                            .find(|entry| entry.id == id)
                                        {
                                            tx.copy(
                                                caelum_core::application::presentation::entry_text(
                                                    entry,
                                                ),
                                            );
                                        }
                                    })
                                    .build(k)
                                    .key(format!("copy-response-{index}-{}", entry.id)),
                            ),
                    )
                    .child(
                        Markdown::new(caelum_core::application::presentation::entry_text(entry))
                            .selectable(true)
                            .streaming(!entry.complete && session.working())
                            .reveal(if reduced { 0.0 } else { 0.12 })
                            .on_link_tx(move |s: &mut Workspace, url, tx| {
                                crate::panels::open_link(s, index, url, tx)
                            })
                            .on_link_secondary(move |s: &mut Workspace, url, at, tx| {
                                crate::context::link(s, index, url, at, tx)
                            })
                            .render(&theme::markdown(k)),
                    );
                if entry.complete {
                    let agent = entry.metadata["agent"].as_str().map(|id| {
                        live.app
                            .choices
                            .get(&session.scope)
                            .and_then(|choices| choices.agents.iter().find(|agent| agent.id == id))
                            .map_or(id, |agent| agent.name.as_str())
                    });
                    let duration = entry.metadata["outputDurationMs"]
                        .as_u64()
                        .map(caelum_core::application::presentation::duration);
                    let details = match (agent, duration.as_deref()) {
                        (Some(agent), Some(duration)) => Some(format!("{agent} · {duration}")),
                        (Some(agent), None) => Some(agent.to_owned()),
                        (None, Some(duration)) => Some(duration.to_owned()),
                        (None, None) => None,
                    };
                    if let Some(details) = details {
                        item = item.child(
                            text(details)
                                .key(format!("output-details-{index}-{}", entry.id))
                                .font(k.ty.small)
                                .color(k.c.text_muted),
                        );
                    }
                }
            }
            EntryKind::Tool | EntryKind::Reasoning => {
                item = item.child(crate::activity::tool(s, k, index, entry));
            }
            EntryKind::Compaction => {
                item = item.child(crate::activity::compaction(s, k, index, entry));
            }
            EntryKind::Status => {
                item = item.child(
                    text(caelum_core::application::presentation::entry_text(entry))
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                );
            }
        }
        for media in entry.media.iter().filter(|_| entry.kind != EntryKind::User) {
            item = item.child(crate::media::thumbnail(s, k, &session.scope.project, media));
        }
        return content.child(crate::context::message(
            item,
            caelum_core::application::presentation::entry_text(entry),
            index,
            format!("live-entry-{index}-{}", entry.id),
        ));
    }
    if let Some(error) = &session.error {
        content = content.child(ui::notice("Session needs attention", error, k));
    }
    if let Some(prompt) = &session.failed_prompt {
        let prompt = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(8.0)
                .child(
                    text(format!("Unsent message\n{prompt}"))
                        .font(k.ty.ui)
                        .color(k.c.text_base)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    Button::new("Restore unsent message")
                        .variant(ButtonVariant::Outline)
                        .disabled(
                            !live.drafts[index].text.is_empty()
                                || !session.attachments.is_empty()
                                || session.attaching > 0,
                        )
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            if live.drafts[index].text.is_empty()
                                && live.app.sessions[index].attachments.is_empty()
                            {
                                let (text, attachments) =
                                    caelum_core::application::split_context(&prompt);
                                live.drafts[index] = TextEdit::new(text);
                                live.app.sessions[index].attachments = attachments;
                                live.drafts[index].focused = true;
                                live.app.sessions[index].failed_prompt = None;
                            }
                        })
                        .build(k)
                        .key("restore-message")
                        .self_start(),
                ),
        );
    }
    for prompt in live.app.approvals.iter().filter(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && (p.session_id.is_none() || p.session_id == session.remote_id)
    }) {
        let allow = prompt.clone();
        let deny = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(10.0)
                .padding(16.0)
                .radius(10.0)
                .border(1.0, k.c.border_focus)
                .child(
                    text(prompt.summary.clone())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .wrap(),
                )
                .child(
                    text(prompt.description.clone())
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(
                            Button::new("Allow once")
                                .variant(ButtonVariant::Outline)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(allow.clone(), ApprovalDecision::AllowOnce)
                                })
                                .build(k)
                                .key(format!("allow-once-{}", prompt.request_id)),
                        )
                        .child(
                            Button::new("Deny")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(deny.clone(), ApprovalDecision::Deny)
                                })
                                .build(k)
                                .key(format!("deny-{}", prompt.request_id)),
                        ),
                ),
        );
    }
    content = content.child(crate::questions::view(s, k, index));
    content
}

pub fn composer(s: &Workspace, k: &Kit, padding: f32, wide: bool, layers: &mut Vec<El>) -> El {
    if live(s).commands.side_owner == live(s).app.selected && live(s).commands.side_owner.is_some()
    {
        return crate::commands::side_view(s, k, padding);
    }
    let live = live(s);
    let Some(index) = live.app.selected else {
        return col();
    };
    let session = &live.app.sessions[index];
    let draft = &live.drafts[index];
    let queued = live.app.will_queue(index);
    let input = text_input(draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .label("Message")
        .placeholder(
            "Use / for commands, @ for files, # for PRs.",
            k.c.text_muted,
        )
        .help(if queued {
            "Enter to queue · Shift+Enter for a new line"
        } else {
            "Enter to send · Shift+Enter for a new line"
        })
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .edit_menu_changed(
            format!("live-draft-{index}"),
            move |s| live_mut(s).drafts.get_mut(index),
            crate::completion::changed,
        )
        .on_input_key(on_key)
        .on_submit(Workspace::send)
        .on_input_paste(move |s: &mut Workspace, content, _| {
            let Some(image) = content.image() else {
                return false;
            };
            live_mut(s)
                .app
                .attach_image(index, "Pasted image.png".into(), image.png().to_vec());
            true
        })
        .on_drop_files(move |s: &mut Workspace, paths, _| {
            let project = live_mut(s).app.sessions[index].scope.project.clone();
            for path in paths {
                match path.strip_prefix(&project) {
                    Ok(path) => live_mut(s)
                        .app
                        .attach_file(index, path.to_string_lossy().into_owned()),
                    Err(_) => {
                        live_mut(s).app.sessions[index].error =
                            Some("Drop files from this session’s project".into())
                    }
                }
            }
        });
    let submit = Button::new(if queued { "Queue" } else { "Send" })
        .icon(if queued { &ui::QUEUE } else { &SEND })
        .variant(ButtonVariant::Contrast)
        .disabled(
            !session.can_submit()
                || (!queued && !session.can_send())
                || (draft.text.trim().is_empty() && session.attachments.is_empty()),
        )
        .on_press_tx(move |s: &mut Workspace, tx| {
            if live_mut(s).app.selected == Some(index) {
                s.send(tx);
            }
        })
        .build(k)
        .key("send")
        .label(if queued {
            "Queue message"
        } else {
            "Send message"
        })
        .help(if queued {
            "Queue message · Enter\nRuns after earlier work · Shift+Enter for a new line"
        } else {
            "Send message · Enter\nShift+Enter for a new line"
        });
    let actions = row()
        .key("composer-actions")
        .center_y()
        .justify_end()
        .gap(6.0)
        .children((session.working() || session.stopping).then(|| {
            Button::new(if session.stopping {
                "Stopping…"
            } else {
                "Stop"
            })
            .icon(&STOP)
            .variant(ButtonVariant::GhostMuted)
            .disabled(session.stopping)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                live.schedule();
                live.app.cancel(index);
            })
            .build(k)
            .px(8.0)
            .key("stop")
            .label("Stop generation")
            .help("Stop generation and pause the queue · Escape")
        }))
        .child(submit);
    let pickers = pickers(s, k, layers).min_w(0.0);
    let usage_label = wide || rune::memo::window_layout("main-pane").is_some_and(|r| r[2] > 440.0);
    let usage = crate::activity::context(s, k, index, usage_label);
    let controls = if wide {
        row()
            .center_y()
            .gap(12.0)
            .child(pickers.grow(1.0))
            .child(usage)
            .child(actions)
    } else {
        col().gap(8.0).child(pickers.w_full()).child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(usage)
                .child(rune::el().grow(1.0))
                .child(actions),
        )
    }
    .key("composer-controls")
    .w_full();
    let activity = crate::activity::session_activity(s, index);
    let has_queue = !session.queue.is_empty() || !session.inbox.items.is_empty();
    let status =
        row()
            .w_full()
            .flex_wrap()
            .center_y()
            .gap(6.0)
            .children(activity.as_ref().map(|a| {
                crate::activity::indicator(a, session.options.agent.as_deref(), k).grow(1.0)
            }))
            .children(has_queue.then(|| crate::panels::queue_summary(s, k, index)));
    ui::composer(
        input,
        controls,
        col()
            .key(format!("composer-context-{index}"))
            .w_full()
            .gap(6.0)
            .children((activity.is_some() || has_queue).then_some(status))
            .children((!session.shells.is_empty()).then(|| crate::activity::shells(s, k, index)))
            .child(crate::panels::composer_context(s, k, index)),
        padding,
        k,
    )
}

fn pickers(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let live = live(s);
    let maximum = rune::memo::window_layout("composer-options").map_or(220.0, |r| r[2].max(80.0));
    let mut content = row()
        .key("composer-options")
        .flex_wrap()
        .gap(6.0)
        .center_y();
    let Some(index) = live.app.selected else {
        return content;
    };
    let session = &live.app.sessions[index];
    let choices = live.app.choices.get(&session.scope);
    content = content.child(
        Select::new::<&str>("model", &[])
            .current(
                choices
                    .and_then(|c| {
                        c.models
                            .iter()
                            .find(|m| Some(&m.id) == session.options.model.as_ref())
                    })
                    .map(|m| m.name.as_str())
                    .or(session.options.model.as_deref()),
            )
            .placeholder("Provider default")
            .label("Model")
            .prominent()
            .placement(Placement::TopStart)
            .open(live.models.is_open())
            .disabled(session.archived || session.archiving)
            .on_toggle(|s: &mut Workspace| crate::models::open(s, None))
            .trigger(k)
            .max_w(maximum.min(220.0))
            .min_w(0.0),
    );
    if let Some(choices) = choices {
        let model_id = session
            .options
            .model
            .as_ref()
            .or(choices.default_model.as_ref());
        if let Some(model) = choices
            .models
            .iter()
            .find(|model| Some(&model.id) == model_id)
            && !model.efforts.is_empty()
        {
            let default = model
                .default_effort
                .as_deref()
                .and_then(|id| model.efforts.iter().find(|e| e.id == id))
                .map(choice_label);
            let mut efforts = vec![
                Choice::new("", "Model default").detail(default.unwrap_or("Let the model choose")),
            ];
            efforts.extend(model.efforts.iter().map(effort_choice));
            let current = session.options.effort.as_deref().unwrap_or("");
            let display = model
                .efforts
                .iter()
                .find(|e| e.id == current)
                .map(choice_label)
                .or(default.filter(|_| current.is_empty()))
                .unwrap_or(if current.is_empty() {
                    "Default"
                } else {
                    current
                });
            content = content.child(
                Select::choices("effort", efforts)
                    .current(Some(current))
                    .display(display)
                    .label("Reasoning effort")
                    .icon(&ui::EFFORT)
                    .menu_description("Depth of reasoning for the next message")
                    .width(280.0)
                    .placement(Placement::TopStart)
                    .open(live.menu == Some(("effort", index)))
                    .disabled(session.archived || session.archiving)
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "effort"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        live.app
                            .set_effort(index, (!value.is_empty()).then(|| value.into()));
                        live.menu = None;
                        live.focus();
                    })
                    .build(k, layers)
                    .max_w(maximum),
            );
        }
        if !choices.agents.is_empty() {
            let mut agents =
                vec![Choice::new("", "Default agent").detail("Use the provider's default agent")];
            agents.extend(
                choices
                    .agents
                    .iter()
                    .map(|agent| Choice::new(&agent.id, choice_label(agent))),
            );
            content = content.child(
                Select::choices("agent", agents)
                    .current(Some(session.options.agent.as_deref().unwrap_or("")))
                    .label("Agent")
                    .icon(&ui::AGENT)
                    .menu_description("Agent for the next message")
                    .width(280.0)
                    .placement(Placement::TopStart)
                    .open(live.menu == Some(("agent", index)))
                    .disabled(session.archived || session.archiving)
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "agent"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        live.app.sessions[index].options.agent =
                            (!value.is_empty()).then(|| value.into());
                        live.app.options_changed(index);
                        live.menu = None;
                        live.focus();
                    })
                    .build(k, layers)
                    .max_w(maximum),
            );
        }
    }
    content
}

fn choice_label(choice: &caelum_protocol::ProviderChoice) -> &str {
    if choice.name != choice.id {
        return &choice.name;
    }
    match choice.name.as_str() {
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Maximum",
        "build" => "Build",
        "plan" => "Plan",
        _ => &choice.name,
    }
}

fn effort_choice(effort: &caelum_protocol::ProviderChoice) -> Choice {
    let choice = Choice::new(&effort.id, choice_label(effort));
    match effort_description(&effort.id) {
        Some(description) => choice.detail(description),
        None => choice,
    }
}

fn effort_description(id: &str) -> Option<&'static str> {
    match id.to_ascii_lowercase().as_str() {
        "none" => Some("Respond without extra reasoning"),
        "minimal" | "low" => Some("Less reasoning, quicker responses"),
        "medium" => Some("Balance reasoning depth and speed"),
        "high" => Some("Think more deeply about complex tasks"),
        "xhigh" | "max" => Some("Use the maximum reasoning effort for complex tasks"),
        _ => None,
    }
}

pub fn session_actions(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(index) = live.session_menu else {
        return col();
    };
    let session = &live.app.sessions[index];
    let mut content = col()
        .w_full()
        .gap(4.0)
        .child(ui::caption(&session.title, k).truncate().padding(8.0))
        .child(ui::divider(k))
        .child(
            ui::choice(
                "Refresh session",
                if session.loading {
                    "Reconnecting and loading history…"
                } else if session.remote_id.is_none() {
                    "Available after your first message"
                } else {
                    "Reconnect and load the latest history"
                },
                false,
                k,
            )
            .disabled(session.remote_id.is_none() || session.loading || session.submitting)
            .opacity(
                if session.remote_id.is_none() || session.loading || session.submitting {
                    0.5
                } else {
                    1.0
                },
            )
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.refresh(index);
                close_session_menu(s);
            })
            .role("menuitem")
            .focus_highlight(k.c.hover)
            .key("refresh-history"),
        );
    let archive_disabled = archive_disabled(live, index);
    content = content.child(
        ui::choice(
            if session.archived {
                "Restore session"
            } else {
                "Archive session"
            },
            if archive_disabled {
                "Finish active work, requests and queued messages first"
            } else {
                "Keep its history and return to it later"
            },
            false,
            k,
        )
        .disabled(archive_disabled)
        .role("menuitem")
        .focus_highlight(k.c.hover)
        .opacity(if archive_disabled { 0.5 } else { 1.0 })
        .on_press(move |s: &mut Workspace| {
            let archived = crate::live::live(s).app.sessions[index].archived;
            live_mut(s).archive(index, !archived);
        })
        .key("archive-session"),
    );
    if session.archived {
        content = content.child(
            ui::choice(
                if session.deleting {
                    "Deleting session…"
                } else {
                    "Delete permanently…"
                },
                "Delete this session and its provider history",
                false,
                k,
            )
            .disabled(archive_disabled || session.deleting)
            .role("menuitem")
            .focus_highlight(k.c.hover)
            .opacity(if archive_disabled || session.deleting {
                0.5
            } else {
                1.0
            })
            .on_press(move |s: &mut Workspace| {
                live_mut(s).request_delete_session(index);
            })
            .key("delete-session"),
        );
    }
    content
}

pub(crate) fn archive_disabled(live: &Live, index: usize) -> bool {
    let session = &live.app.sessions[index];
    session.working()
        || session.archiving
        || session.deleting
        || !session.queue.is_empty()
        || !session.inbox.items.is_empty()
        || live.app.approvals.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && p.session_id == session.remote_id
        })
        || live.app.questions.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && Some(&p.session_id) == session.remote_id.as_ref()
        })
}

pub fn close_session_menu(s: &mut Workspace) {
    let l = live_mut(s);
    if let Some(index) = l.session_menu.take() {
        l.focus_key = Some(format!("session-actions-{index}"));
    }
}

/// Rune's Tx exposes focus only during key handlers. Read the presented focus after
/// navigation (including Tab, pointer and accessibility moves), never from a timer.
pub fn presented_focus(rt: &rune::Runtime<Workspace>) -> Option<Option<String>> {
    let l = rt.state.live.as_ref()?;
    // Pinned Rune commits pointer, keyboard and accessibility focus changes. Hover
    // paint and animation frames don't: inspecting them copies the entire UI tree
    // just to rediscover the same focus. Also observe blur when an exit is pruned.
    let observation = (rt.stats.commits, rt.has_focus());
    if l.focus_observation.replace(Some(observation)) == Some(observation) {
        return None;
    }
    let focused = observation
        .1
        .then(|| {
            rt.elements()
                .into_iter()
                .find(|e| e.focused && !e.exiting)
                .and_then(|e| e.key)
        })
        .flatten();
    (l.focus_key != focused).then_some(focused)
}

pub fn frame_drawn(rt: &rune::Runtime<Workspace>) {
    if let Some(focused) = presented_focus(rt)
        && let Some(handle) = &rt.state.handle
    {
        handle.update(move |s, _| {
            live_mut(s).focus_key = focused;
        });
    }
}

pub fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let l = live(s);
    if let Some(index) = l.app.last_archived {
        let session = &l.app.sessions[index];
        let width =
            rune::memo::window_layout("main-pane").map_or(480.0, |r| (r[2] - 32.0).min(480.0));
        let toast = row()
            .key("archive-toast")
            .role("status")
            .label("Session archived")
            .w(width)
            .padding(10.0)
            .gap(10.0)
            .center_y()
            .on_hover_change(|s: &mut Workspace, inside, _| live_mut(s).archive_hovered = inside)
            .child(icon(&ui::ARCHIVE, 15.0).color(k.c.icon_muted))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(3.0)
                    .child(
                        text("Session archived")
                            .font(k.ty.ui_medium)
                            .color(k.c.text_base),
                    )
                    .child(ui::caption(&session.title, k).truncate())
                    .children(
                        session
                            .error
                            .as_ref()
                            .map(|e| ui::caption(e, k).color(k.c.danger.fg)),
                    ),
            )
            .child(
                Button::new(if session.archiving {
                    "Restoring…"
                } else {
                    "Undo"
                })
                .variant(ButtonVariant::GhostMuted)
                .disabled(session.archiving)
                .on_press(move |s: &mut Workspace| {
                    let l = live_mut(s);
                    if l.app.last_archived == Some(index) {
                        l.app.undo_archive();
                        l.sync_editors();
                    }
                })
                .build(k)
                .key("undo-archive"),
            )
            .child(
                quiet_button(&glyphs::CLOSE, "Dismiss archive notice")
                    .on_press(move |s: &mut Workspace| live_mut(s).app.dismiss_archive(index))
                    .build(k)
                    .key("dismiss-archive"),
            );
        let x =
            rune::memo::window_layout("main-pane").map_or(16.0, |r| r[0] + (r[2] - width) / 2.0);
        let bottom = rune::memo::window_layout("message-composer")
            .zip(rune::memo::window_layout("shell"))
            .map_or(18.0, |(composer, shell)| shell[3] - composer[1] + 12.0);
        layers.push(rune_kit::popover::raised(k, toast, 12.0).pin(
            Some(x),
            None,
            None,
            Some(bottom),
        ));
    }
    if let Some(index) = l.session_menu {
        ui::popup(
            "session-menu",
            &format!("session-actions-{index}"),
            session_actions(s, k).w(300.0),
            Placement::BottomStart,
            close_session_menu,
            k,
            layers,
        );
    }
}

pub fn delete_confirmation(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let Some(index) = live(s).delete_confirmation else {
        return;
    };
    let Some(session) = live(s).app.sessions.get(index) else {
        return;
    };
    let title = session.title.clone();
    let description = if session.remote_id.is_some() {
        format!(
            "Delete “{title}”, its provider history and any child sessions? This cannot be undone."
        )
    } else {
        format!(
            "Permanently remove “{title}” and its unsent draft and attachments? This cannot be undone."
        )
    };
    Dialog::new("delete-session", "Delete session permanently?")
        .open(true)
        .click_outside(false)
        .width(460.0)
        .fit()
        .on_close(|s: &mut Workspace| live_mut(s).cancel_delete_session())
        .child(
            col()
                .w_full()
                .gap(16.0)
                .px(16.0)
                .pb(16.0)
                .child(
                    text(description)
                        .key("delete-session-description")
                        .font(k.ty.ui.lh(20.0))
                        .color(k.c.text_muted)
                        .wrap()
                        .w_full(),
                )
                .child(
                    row()
                        .w_full()
                        .justify_end()
                        .gap(8.0)
                        .child(
                            crate::ui::Button::new("Cancel")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(|s: &mut Workspace| live_mut(s).cancel_delete_session())
                                .build(k)
                                .key("cancel-delete-session"),
                        )
                        .child(
                            crate::ui::Button::new("Delete permanently")
                                .variant(ButtonVariant::Contrast)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s).confirm_delete_session(index)
                                })
                                .build(k)
                                .key("confirm-delete-session"),
                        ),
                ),
        )
        .build(k, layers);
}

fn toggle_menu(s: &mut Workspace, name: &'static str) {
    let live = live_mut(s);
    let next = live.app.selected.map(|index| (name, index));
    live.menu = if live.menu == next { None } else { next };
}

pub fn open_effort(s: &mut Workspace, index: usize) {
    let live = live_mut(s);
    live.menu = Some(("effort", index));
    live.drafts[index].focused = false;
}

#[cfg(test)]
mod tests {
    use super::effort_description;

    #[test]
    fn max_effort_has_a_description() {
        assert_eq!(
            effort_description("max"),
            Some("Use the maximum reasoning effort for complex tasks")
        );
    }
}

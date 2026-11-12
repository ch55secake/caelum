//! Rune adapter: editors and disclosure state stay here; application requests,
//! provider activity and history reconciliation live in caelum-core.

mod composer;
mod events;
mod menus;
mod patches;
#[cfg(test)]
mod polling_tests;
mod sidebar;
mod transcript;

pub use composer::composer;
pub use composer::open_effort;
pub(crate) use menus::archive_disabled;
pub use menus::close_session_menu;
pub use menus::delete_confirmation;
pub use menus::overlays;
pub use sidebar::on_key as sidebar_key;
pub use sidebar::sidebar;
pub use transcript::transcript;

use std::{cell::Cell, collections::HashSet, rc::Rc};

use caelum_core::application::{Event, Runtime, Scope, Workspace as Application};
use rune::{FileDialogOptions, ScrollPos, TextEdit, Tx, Waker};

use crate::Workspace;

pub struct Live {
    pub feed: sidebar::State,
    pub context: crate::context::State,
    pub checkouts: crate::checkouts::State,
    pub completion: crate::completion::Menu,
    pub composer: crate::composer::State,
    pub commands: crate::commands::State,
    pub models: crate::models::Picker,
    pub notifications: crate::notifications::State,
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
    patch_inputs: patches::Inputs,
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
            feed: Default::default(),
            context: Default::default(),
            checkouts: Default::default(),
            completion: Default::default(),
            composer: Default::default(),
            commands: Default::default(),
            models: Default::default(),
            notifications: Default::default(),
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
            patch_inputs: Default::default(),
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
            self.composer.editors.push(Default::default());
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
        self.feed.project = None;
        self.feed.close();
        self.panels.sessions.clear();
        self.panels.archived = false;
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
        if crate::questions::focus(self) {
            return;
        }
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
        self.forms.apply(&event);
        let questions_changed = matches!(&event, Event::Questions(_));
        if let Some(panel) = &mut self.shell_panel {
            panel.sync(&self.app);
        }
        let selected = self.app.selected;
        let was_archived = selected.is_some_and(|i| self.app.sessions[i].archived);
        self.app.apply(event);
        if questions_changed {
            self.forms.sync(&self.app.questions);
        }
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
        changed |= self.feed.poll(
            &mut self.app,
            &self.panels.sessions.text,
            self.panels.archived,
            tx,
            !self.fixture,
        );
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
        changed |= self.apply_events(events);
        changed |= self.app.acknowledge();
        changed |= self.sync_patches();
        if self.last_selected != self.app.selected
            && let Some(index) = self.app.selected
        {
            let scope = self.app.sessions[index].scope.clone();
            if !self.app.choices.contains_key(&scope) {
                // Retry missing catalogues on navigation, not on every idle poll.
                self.app.refresh_choices_for(scope);
            }
        }
        if self.runtime.is_some() {
            changed |= crate::notifications::dispatch(self);
            self.feed.discover(&mut self.app);
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
                if let caelum_core::application::Command::Notify(notification) = command {
                    if let Err(error) = self.notifications.send(id, notification) {
                        eprintln!("Notification: {error}");
                        self.apply(Event::Completed(id, Err(error)));
                    }
                    continue;
                }
                if let Err(error) = self.runtime.as_mut().unwrap().submit(id, command) {
                    self.apply(Event::Completed(id, Err(error)));
                    changed = true;
                }
            }
        }
        self.sync_editors();
        self.forms.sync(&self.app.questions);
        if crate::questions::sync(self, tx) {
            self.menu = None;
            changed = true;
        }
        changed |= crate::composer::sync(self, tx);
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

/// Rune's Tx exposes focus only during key handlers. Read the presented focus after
/// navigation (including Tab, pointer and accessibility moves), never from a timer.
pub fn presented_focus(rt: &rune::Runtime<Workspace>) -> Option<Option<String>> {
    let l = rt.state.live.as_ref()?;
    // Rune commits pointer, keyboard and accessibility focus changes. Paint-only
    // frames keep the same focus. Also observe blur when an exit is pruned.
    let observation = (rt.stats.commits, rt.has_focus());
    if l.focus_observation.replace(Some(observation)) == Some(observation) {
        return None;
    }
    let focused = rt.presented_focus_key();
    (l.focus_key.as_deref() != focused).then(|| focused.map(str::to_owned))
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

#[cfg(test)]
mod tests {
    use super::composer::effort_description;

    #[test]
    fn max_effort_has_a_description() {
        assert_eq!(
            effort_description("max"),
            Some("Use the maximum reasoning effort for complex tasks")
        );
    }
}

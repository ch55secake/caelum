//! Rune adapter: editors and disclosure state stay here; application requests,
//! provider activity and history reconciliation live in caelum-core.

use std::{cell::Cell, collections::HashSet, rc::Rc};

use caelum_core::application::{
    EntryKind, Event, Provider, Runtime, Scope, Workspace as Application,
};
use caelum_protocol::ApprovalDecision;
use rune::{
    El, FileDialogOptions, ScrollPos, ScrollSpec, TextEdit, TextStyle, Tx, Waker, col, icon, row,
    text, text_input,
};
use rune_kit::shell::{RowStatus, Sidebar, SidebarHeader, SidebarRow};
use rune_kit::{Button, ButtonVariant, Kit, Markdown, Placement, Select, glyphs};

use crate::{PLUS, SEND, STOP, Workspace, on_key, quiet_button, theme, ui};

pub struct Live {
    pub app: Application,
    pub drafts: Vec<TextEdit>,
    pub panels: crate::panels::Panels,
    pub forms: crate::questions::Forms,
    pub evaluation: bool,
    pub fixture: bool,
    evaluation_ticks: usize,
    runtime: Option<Runtime>,
    store_lock: Option<std::fs::File>,
    expanded: HashSet<(usize, String)>,
    menu: Option<&'static str>,
    reading: Vec<Rc<Cell<ScrollPos>>>,
    last_selected: Option<usize>,
    history_lengths: Vec<usize>,
    last_screen: crate::panels::Screen,
    last_completion: Option<(String, String)>,
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
        self.runtime = Some(Runtime::new(move || wake()));
    }

    pub fn new(project: Option<String>) -> Self {
        let mut app = Application::default();
        if let Some(project) = project {
            app.open_project(project);
        }
        Self {
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
            menu: None,
            reading: Vec::new(),
            last_selected: None,
            history_lengths: Vec::new(),
            last_screen: crate::panels::Screen::Conversation,
            last_completion: None,
        }
    }

    fn sync_editors(&mut self) {
        while self.drafts.len() < self.app.sessions.len() {
            let session = &self.app.sessions[self.drafts.len()];
            self.drafts.push(TextEdit::new(&session.draft));
            let mut position = ScrollPos::default();
            if session.reading >= 0.0 {
                position.offset[1] = session.reading;
                position.max[1] = session.reading + 10.0;
            }
            self.reading.push(Rc::new(Cell::new(position)));
        }
    }

    pub fn new_session(&mut self) {
        let Some(project) = self.app.active_project.clone() else {
            return;
        };
        let provider = self
            .app
            .selected
            .map(|i| self.app.sessions[i].scope.provider)
            .unwrap_or(self.app.preferences.default_provider);
        let index = self.app.new_session(Scope { project, provider });
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

    pub fn working(&self) -> bool {
        self.app
            .selected
            .is_some_and(|index| self.app.sessions[index].working())
    }

    pub fn stop(&mut self) {
        if let Some(index) = self.app.selected {
            self.app.cancel(index);
        }
    }

    pub fn send(&mut self, tx: &mut Tx) {
        let Some(index) = self.app.selected else {
            return;
        };
        let draft = self.drafts[index].text.trim().to_owned();
        if let Some(prompt) = draft.strip_prefix("/ask ") {
            if self.app.ask(index, prompt.into()) {
                self.drafts[index].clear();
            }
            return;
        }
        if draft == "/new" {
            self.drafts[index].clear();
            self.new_session();
            return;
        }
        if let Some(command) = draft.strip_prefix('/') {
            let name = command.split_whitespace().next().unwrap_or_default();
            let session = &self.app.sessions[index];
            if !self
                .app
                .provider_commands
                .get(&session.scope)
                .is_some_and(|commands| commands.iter().any(|c| c.name == name))
            {
                self.app.sessions[index].error = Some(format!("Unknown command /{name}"));
                return;
            }
        }
        if self.app.send(index, self.drafts[index].text.clone()) {
            self.drafts[index].clear();
            self.drafts[index].focused = true;
            tx.scroll_to_bottom(format!("live-transcript-{index}"));
        }
    }

    pub fn poll(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
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
        if let Some(index) = self.app.selected {
            let completion = self
                .drafts
                .get(index)
                .and_then(|draft| crate::panels::mention(&draft.text))
                .map(|(_, query)| (self.app.sessions[index].scope.project.clone(), query.into()));
            if completion != self.last_completion {
                self.last_completion = completion.clone();
                if let Some((project, query)) = completion {
                    self.app
                        .task(caelum_core::application::Task::Completions { project, query });
                }
            }
        }
        if let Some(runtime) = &mut self.runtime {
            let events = runtime.drain();
            changed = !events.is_empty();
            for event in events {
                self.app.apply(event);
            }
            for (id, command) in self.app.commands() {
                if let Err(error) = runtime.submit(id, command) {
                    self.app.apply(Event::Completed(id, Err(error)));
                    changed = true;
                }
            }
        }
        self.sync_editors();
        self.forms.sync(&self.app.questions);
        self.panels.sync(&self.app);
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
        changed
    }
}

impl Drop for Live {
    fn drop(&mut self) {
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
    s.live.as_mut().expect("live workspace")
}

pub fn open_project(_: &mut Workspace, tx: &mut Tx) {
    tx.open_folder(
        FileDialogOptions {
            title: "Open a project".into(),
            ..Default::default()
        },
        |s: &mut Workspace, result, _| match result {
            Ok(Some(path)) => live_mut(s)
                .app
                .open_project(path.to_string_lossy().into_owned()),
            Ok(None) => {}
            Err(error) => live_mut(s).app.error = Some(error.to_string()),
        },
    );
}

pub fn sidebar(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let mut sidebar = Sidebar::new()
        .label("Projects and sessions")
        .inset(0.0)
        .row_gap(3.0)
        .header_gap(8.0)
        .header(ui::brand(k))
        .header(
            SidebarHeader::new(if app.opening {
                "Opening project…"
            } else {
                "Projects"
            })
            .action(
                quiet_button(&PLUS, "Open project")
                    .on_press_tx(open_project)
                    .build(k)
                    .key("open-project"),
            )
            .build(k)
            .key("projects-heading"),
        );
    let mut projects = col().w_full().gap(4.0);
    for (index, project) in app
        .projects
        .iter()
        .enumerate()
        .filter(|(_, project)| app.group_path(&project.path) == project.path)
    {
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
                app.sessions
                    .iter()
                    .find(|s| app.group_path(&s.scope.project) == path && !s.archived)
            });
        let checkout = active.map(|s| &s.scope.project).unwrap_or(&path);
        let git = app
            .services
            .checkouts
            .get(checkout)
            .and_then(|c| c.git.as_ref());
        let branch = git.and_then(|g| g.branch.as_deref()).unwrap_or("No branch");
        let detail = if requests > 0 {
            format!("{requests} need attention")
        } else if activity > 0 {
            format!("{activity} working")
        } else {
            git.map(|g| format!("{} changed", g.changes.len()))
                .unwrap_or_default()
        };
        projects = projects.child(
            k.interactive(
                col()
                    .w_full()
                    .gap(5.0)
                    .padding(9.0)
                    .radius(10.0)
                    .fill(if selected {
                        k.c.layer_01
                    } else {
                        rune::Color::TRANSPARENT
                    })
                    .border(
                        1.0,
                        if selected {
                            k.c.border_muted
                        } else {
                            rune::Color::TRANSPARENT
                        },
                    )
                    .child(
                        row()
                            .gap(7.0)
                            .center_y()
                            .child(icon(&ui::FOLDER, 14.0).color(k.c.icon_muted))
                            .child(
                                text(&project.name)
                                    .font(k.ty.ui_medium)
                                    .color(k.c.text_base)
                                    .truncate()
                                    .grow(1.0),
                            ),
                    )
                    .child(
                        text(
                            active
                                .map(|s| s.title.as_str())
                                .unwrap_or("Choose a session"),
                        )
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .truncate()
                        .pl(21.0),
                    )
                    .child(
                        row()
                            .gap(5.0)
                            .center_y()
                            .pl(21.0)
                            .child(icon(&ui::BRANCH, 11.0).color(k.c.icon_muted))
                            .child(
                                text(branch)
                                    .font(k.ty.small)
                                    .color(k.c.text_muted)
                                    .truncate()
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
            .focusable(true)
            .focus_ring(1.0, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                live.app.select_project(&path);
                live.sync_editors();
                live.menu = None;
                live.panels.screen = crate::panels::Screen::Conversation;
            })
            .key(format!("project-{index}")),
        );
    }
    sidebar = sidebar
        .header(
            col()
                .w_full()
                .max_h(190.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(projects),
        )
        .header(
            row()
                .w_full()
                .gap(4.0)
                .child(
                    Button::new("New session")
                        .icon(&PLUS)
                        .variant(ButtonVariant::Outline)
                        .disabled(app.active_project.is_none())
                        .on_press(|s: &mut Workspace| s.new_session())
                        .build(k)
                        .key("new-session")
                        .grow(1.0),
                )
                .child(crate::panels::navigation(s, k)),
        )
        .header(
            SidebarHeader::new(if live(s).panels.archived {
                "Archived sessions"
            } else {
                "Sessions"
            })
            .count(
                app.sessions
                    .iter()
                    .filter(|session| {
                        session.archived == live(s).panels.archived
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
                            .disabled(app.active_project.is_none())
                            .on_press(|s: &mut Workspace| live_mut(s).app.refresh_project())
                            .build(k)
                            .key("refresh-sessions"),
                    ),
            )
            .build(k),
        );
    if live(s).panels.searching_sessions {
        sidebar = sidebar.header(
            text_input(&live(s).panels.sessions, k.ty.ui)
                .key("session-search")
                .label("Search sessions")
                .placeholder("Search sessions · ⌘K", k.c.text_muted)
                .color(k.c.text_base)
                .w_full()
                .padding(6.0)
                .radius(6.0)
                .border(1.0, k.c.border_base)
                .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.sessions),
        );
    }
    for (index, session) in
        app.sessions.iter().enumerate().filter(|(_, session)| {
            session.archived == live(s).panels.archived
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
            .unwrap_or(session.scope.provider.as_str());
        let detail = format!(
            "⑂ {} · {}",
            branch,
            if pending {
                "Needs an answer"
            } else if session.stopping {
                "Stopping…"
            } else if session.working() {
                "Working…"
            } else if session.loading {
                "Loading history…"
            } else {
                session.scope.project.rsplit('/').next().unwrap_or_default()
            }
        );
        let mut item = ui::session_row(&session.title, &detail, app.selected == Some(index), k)
            .on_select(move |s: &mut Workspace, _| {
                let live = live_mut(s);
                live.app.select(index);
                live.panels.screen = crate::panels::Screen::Conversation;
                live.menu = None;
            });
        if pending {
            item = item.status(RowStatus::Attention);
        } else if session.working() {
            item = item.status(RowStatus::Busy);
        }
        if !session.working() && !session.archiving && session.queue.is_empty() {
            item = item.on_close(move |s: &mut Workspace, _| {
                let app = &mut live_mut(s).app;
                app.archive(index, !app.sessions[index].archived);
            });
        }
        sidebar = sidebar.child(ui::selected_row(
            item.build(k).key(format!("live-session-{index}")),
            app.selected == Some(index),
            k,
        ));
    }
    let mut footer = col().w_full().gap(6.0);
    for provider in Provider::ALL {
        if let Some(path) = &app.active_project
            && let Some(error) = app.notices.get(&Scope {
                provider,
                project: path.clone(),
            })
        {
            footer = footer.child(
                text(format!("{}: {error}", provider.as_str()))
                    .font(k.ty.small)
                    .color(k.c.text_muted)
                    .wrap(),
            );
        }
    }
    if let Some(index) = app.last_archived {
        footer = footer.child(
            Button::new("Undo archive")
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut Workspace| live_mut(s).app.archive(index, false))
                .build(k)
                .key("undo-archive"),
        );
    }
    let mut seen = HashSet::new();
    let mut requests = col().w_full().gap(2.0);
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
            requests = requests.child(
                SidebarRow::new(&pr.title)
                    .icon(&ui::BRANCH)
                    .subtitle(format!("#{} · into {}", pr.number, pr.base_ref_name))
                    .height(48.0)
                    .title_style(k.ty.ui)
                    .subtitle_style(k.ty.small)
                    .on_select(move |s: &mut Workspace, _| {
                        crate::panels::open_review(s, project.clone(), url.clone())
                    })
                    .build(k)
                    .key(format!("pull-request-{}", pr.number)),
            );
        }
    }
    footer = footer
        .child(rune::el().w_full().h(1.0).fill(k.c.border_base))
        .child(
            SidebarHeader::new("Pull requests")
                .count(seen.len())
                .build(k),
        )
        .child(
            col()
                .w_full()
                .max_h(150.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(if seen.is_empty() {
                    ui::caption("No pull requests for open sessions", k).padding(8.0)
                } else {
                    requests
                }),
        )
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    Button::new("Settings")
                        .icon(&ui::SETTINGS)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(|s: &mut Workspace| {
                            crate::panels::show(s, crate::panels::Screen::Settings)
                        })
                        .build(k)
                        .key("nav-settings")
                        .grow(1.0),
                )
                .child(
                    quiet_button(&ui::MORE, "Session actions")
                        .on_press(|s: &mut Workspace| {
                            crate::panels::show(s, crate::panels::Screen::Session)
                        })
                        .build(k)
                        .key("session-options"),
                ),
        );
    sidebar
        .footer(footer.max_h(240.0).scroll(ScrollSpec {
            stick_bottom: false,
        }))
        .build(k)
}

pub fn transcript(s: &Workspace, k: &Kit, reduced: bool, padding: f32) -> El {
    if live(s).panels.screen == crate::panels::Screen::Review {
        return crate::panels::view(s, k);
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
    let count = live.app.sessions[index].conversation.entries.len();
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
                            .get(row - 1)
                        {
                            Some(entry) => format!("live-entry-{index}-{}", entry.id).into(),
                            None => format!("live-tail-{index}").into(),
                        }
                    },
                    move |s: &Workspace, row| timeline_row(s, &kit, reduced, padding, index, row),
                ),
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
            content = content.child(
                text("Loading history…")
                    .font(k.ty.small)
                    .color(k.c.text_muted),
            );
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
        if session.conversation.entries.is_empty() && !session.loading {
            content = content.child(ui::empty(
                "A little room to think",
                "Ask a question, describe a change, or @mention a file.",
                k,
            ));
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
                let (message, files) = caelum_core::application::split_context(&entry.text);
                item = item.child(ui::user_message(message, k));
                for file in files {
                    item = item.child(
                        text(format!(
                            "Attached: {}{}",
                            file.path,
                            if file.truncated { " · truncated" } else { "" }
                        ))
                        .font(k.ty.small)
                        .color(k.c.text_muted),
                    );
                }
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
                                text(if entry.complete {
                                    session.scope.provider.as_str()
                                } else {
                                    "Responding…"
                                })
                                .font(k.ty.small)
                                .color(k.c.text_muted),
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
                                            tx.copy(entry.text.clone());
                                        }
                                    })
                                    .build(k),
                            ),
                    )
                    .child(
                        Markdown::new(&entry.text)
                            .selectable(true)
                            .streaming(!entry.complete && session.working())
                            .reveal(if reduced { 0.0 } else { 0.12 })
                            .on_link_tx(move |s: &mut Workspace, url, tx| {
                                crate::panels::open_link(s, index, url, tx)
                            })
                            .render(&theme::markdown(k)),
                    );
            }
            EntryKind::Tool | EntryKind::Reasoning | EntryKind::Compaction => {
                let id = entry.id.clone();
                let expanded = live.expanded.contains(&(index, id.clone()));
                item = item.child(
                    Button::new(if entry.kind == EntryKind::Compaction {
                        format!("Context compaction · {}", entry.status)
                    } else if entry.kind == EntryKind::Reasoning {
                        "Thinking".into()
                    } else if entry.name.is_empty() {
                        "Tool".into()
                    } else {
                        let name = match entry.name.as_str() {
                            "commandExecution" | "bash" | "shell" => "Run",
                            "fileChange" | "apply_patch" | "edit" => "Edit files",
                            "read" | "read_file" => "Read",
                            "webSearch" | "websearch" => "Search",
                            other => other,
                        };
                        if name == "Run" {
                            format!(
                                "Run {}",
                                entry
                                    .input
                                    .lines()
                                    .next()
                                    .unwrap_or_default()
                                    .chars()
                                    .take(100)
                                    .collect::<String>()
                            )
                        } else {
                            format!("{name} · {}", entry.status)
                        }
                    })
                    .icon(if expanded {
                        &glyphs::CHEVRON_DOWN
                    } else {
                        &glyphs::CARET_RIGHT
                    })
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        let expanded = &mut live_mut(s).expanded;
                        let key = (index, id.clone());
                        if !expanded.remove(&key) {
                            expanded.insert(key);
                        }
                    })
                    .build(k)
                    .expanded(expanded)
                    .self_start(),
                );
                if expanded {
                    item = item.child(
                        text(
                            if matches!(entry.kind, EntryKind::Reasoning | EntryKind::Compaction) {
                                entry.text.clone()
                            } else {
                                format!("{}\n{}", entry.input, entry.output)
                            },
                        )
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true)
                        .padding(12.0)
                        .radius(8.0)
                        .fill(k.c.layer_01),
                    );
                }
            }
            EntryKind::Status => {
                item = item.child(
                    text(entry.text.clone())
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                );
            }
        }
        return content.child(item);
    }
    if let Some(error) = &session.error {
        content = content.child(
            text(error.clone())
                .font(k.ty.ui)
                .color(k.c.text_accent)
                .wrap()
                .selectable(true),
        );
    }
    if let Some((question, answer)) = &session.side_question {
        content = content
            .child(
                text(format!("Side question: {question}"))
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base)
                    .wrap(),
            )
            .child(
                Markdown::new(if session.generating {
                    "Thinking…"
                } else {
                    answer
                })
                .selectable(true)
                .render(&theme::markdown(k)),
            );
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
    let live = live(s);
    let Some(index) = live.app.selected else {
        return col();
    };
    let session = &live.app.sessions[index];
    let draft = &live.drafts[index];
    let input = text_input(draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .key(format!("live-draft-{index}"))
        .label("Message")
        .placeholder("Message your session…", k.c.text_muted)
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .min_h(56.0)
        .max_h(180.0)
        .on_edit(move |s: &mut Workspace| &mut live_mut(s).drafts[index])
        .on_input_key(on_key)
        .on_submit(Workspace::send)
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
    let submit = Button::new("")
        .icon(if session.working() { &STOP } else { &SEND })
        .variant(ButtonVariant::Submit)
        .disabled(
            session.stopping
                || (!session.working()
                    && (!session.can_send()
                        || (draft.text.trim().is_empty() && session.attachments.is_empty()))),
        )
        .on_press_tx(|s: &mut Workspace, tx| {
            if live_mut(s).working() {
                live_mut(s).stop();
            } else {
                s.send(tx);
            }
        })
        .build(k)
        .key("send")
        .label(if session.working() {
            "Stop generation"
        } else {
            "Send message"
        })
        .size(32.0, 32.0);
    let mut controls = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            quiet_button(&ui::ATTACH, "Attach a file")
                .on_press_tx(crate::panels::attach_dialog)
                .build(k)
                .key("attach-file"),
        )
        .child(rune::el().grow(1.0));
    if session.working() && !draft.text.trim().is_empty() {
        controls = controls.child(
            Button::new("Queue")
                .variant(ButtonVariant::Outline)
                .on_press_tx(Workspace::send)
                .build(k)
                .key("queue-message"),
        );
    }
    if wide {
        controls = controls.child(
            text("↵ Send · ⇧↵ Newline")
                .font(k.ty.small)
                .color(k.c.text_muted),
        );
    }
    let pickers = pickers(s, k, layers).h(24.0).min_w(0.0).scroll_x();
    let controls = if wide {
        row()
            .w_full()
            .gap(4.0)
            .center_y()
            .child(pickers.grow(1.0))
            .child(controls.child(submit).w(220.0))
    } else {
        col()
            .w_full()
            .gap(2.0)
            .child(pickers.w_full())
            .child(controls.child(submit))
    };
    ui::composer(
        input,
        controls,
        crate::panels::composer_context(s, k, index),
        draft.focused,
        padding,
        k,
    )
}

fn pickers(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let live = live(s);
    let mut content = row().gap(2.0).center_y();
    let Some(index) = live.app.selected else {
        return content;
    };
    let session = &live.app.sessions[index];
    content = content.child(
        Select::new("provider", &["codex", "opencode"])
            .current(Some(session.scope.provider.as_str()))
            .label("Provider")
            .placement(Placement::TopStart)
            .open(live.menu == Some("provider"))
            .disabled(session.remote_id.is_some() || !session.can_send())
            .on_toggle(|s: &mut Workspace| toggle_menu(s, "provider"))
            .on_close(|s: &mut Workspace| live_mut(s).menu = None)
            .on_choose(move |s: &mut Workspace, value, _| {
                let live = live_mut(s);
                let session = &mut live.app.sessions[index];
                session.scope.provider = if value == "codex" {
                    Provider::Codex
                } else {
                    Provider::OpenCode
                };
                session.options = live
                    .app
                    .preferences
                    .providers
                    .get(session.scope.provider.as_str())
                    .cloned()
                    .unwrap_or_default();
                if let Some(choices) = live.app.choices.get(&session.scope)
                    && session.options.model.is_none()
                {
                    session.options.model = choices.default_model.clone();
                }
                live.app.options_changed(index);
                live.menu = None;
            })
            .build(k, layers),
    );
    if let Some(choices) = live.app.choices.get(&session.scope) {
        let mut models: Vec<_> = choices
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect();
        if let Some(favorites) = live
            .app
            .preferences
            .favorite_models
            .get(session.scope.provider.as_str())
        {
            models.sort_by_key(|model| !favorites.iter().any(|favorite| favorite == model));
        }
        content = content.child(
            Select::new("model", &models)
                .current(session.options.model.as_deref())
                .placeholder("Provider default")
                .label("Model")
                .placement(Placement::TopStart)
                .open(live.menu == Some("model"))
                .disabled(session.archived || session.archiving)
                .on_toggle(|s: &mut Workspace| toggle_menu(s, "model"))
                .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                .on_choose(move |s: &mut Workspace, value, _| {
                    let live = live_mut(s);
                    let session = &mut live.app.sessions[index];
                    session.options.model = Some(value.into());
                    session.options.effort = live
                        .app
                        .choices
                        .get(&session.scope)
                        .and_then(|c| c.models.iter().find(|model| model.id == value))
                        .and_then(|model| model.default_effort.clone());
                    live.app.options_changed(index);
                    live.menu = None;
                })
                .build(k, layers),
        );
        if let Some(model) = choices
            .models
            .iter()
            .find(|model| Some(&model.id) == session.options.model.as_ref())
        {
            let efforts: Vec<_> = model
                .efforts
                .iter()
                .map(|effort| effort.id.as_str())
                .collect();
            if !efforts.is_empty() {
                content = content.child(
                    Select::new("effort", &efforts)
                        .current(
                            session
                                .options
                                .effort
                                .as_deref()
                                .or(model.default_effort.as_deref()),
                        )
                        .label("Reasoning effort")
                        .placement(Placement::TopStart)
                        .open(live.menu == Some("effort"))
                        .disabled(session.archived || session.archiving)
                        .on_toggle(|s: &mut Workspace| toggle_menu(s, "effort"))
                        .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                        .on_choose(move |s: &mut Workspace, value, _| {
                            let live = live_mut(s);
                            live.app.sessions[index].options.effort = Some(value.into());
                            live.app.options_changed(index);
                            live.menu = None;
                        })
                        .build(k, layers),
                );
            }
        }
        let agents: Vec<_> = choices
            .agents
            .iter()
            .map(|agent| agent.id.as_str())
            .collect();
        if !agents.is_empty() {
            content = content.child(
                Select::new("agent", &agents)
                    .current(session.options.agent.as_deref())
                    .placeholder("Provider default")
                    .label("Agent")
                    .placement(Placement::TopStart)
                    .open(live.menu == Some("agent"))
                    .disabled(session.archived || session.archiving)
                    .on_toggle(|s: &mut Workspace| toggle_menu(s, "agent"))
                    .on_close(|s: &mut Workspace| live_mut(s).menu = None)
                    .on_choose(move |s: &mut Workspace, value, _| {
                        let live = live_mut(s);
                        live.app.sessions[index].options.agent = Some(value.into());
                        live.app.options_changed(index);
                        live.menu = None;
                    })
                    .build(k, layers),
            );
        }
    }
    content
}

pub fn session_actions(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let mut content = col().w_full().gap(12.0).child(
        Button::new(if live.panels.archived {
            "Show active sessions"
        } else {
            "Show archived sessions"
        })
        .variant(ButtonVariant::GhostMuted)
        .on_press(|s: &mut Workspace| {
            let panels = &mut live_mut(s).panels;
            panels.archived = !panels.archived;
            panels.overlay = None;
        })
        .build(k)
        .key("toggle-archived"),
    );
    let Some(index) = live.app.selected else {
        return content;
    };
    let session = &live.app.sessions[index];
    content = content
        .child(ui::section(&session.title, &session.scope.project, k))
        .child(
            Button::new("Refresh / reconnect session")
                .variant(ButtonVariant::Outline)
                .disabled(session.remote_id.is_none() || session.loading || session.submitting)
                .on_press(move |s: &mut Workspace| live_mut(s).app.refresh(index))
                .build(k)
                .key("refresh-history"),
        );
    content = content.child(
        Button::new(if session.archived {
            "Restore session"
        } else {
            "Archive session"
        })
        .variant(ButtonVariant::GhostMuted)
        .disabled(session.working() || session.archiving || !session.queue.is_empty())
        .on_press(move |s: &mut Workspace| {
            let archived = crate::live::live(s).app.sessions[index].archived;
            live_mut(s).app.archive(index, !archived);
        })
        .build(k)
        .key("archive-session"),
    );
    content = content.child(
        Button::new("Save as provider defaults")
            .variant(ButtonVariant::Outline)
            .on_press(move |s: &mut Workspace| {
                let app = &mut live_mut(s).app;
                let session = &app.sessions[index];
                app.preferences.providers.insert(
                    session.scope.provider.as_str().into(),
                    session.options.clone(),
                );
                app.preferences.default_provider = session.scope.provider;
            })
            .build(k),
    );
    if let Some(model) = &session.options.model {
        let provider = session.scope.provider.as_str().to_owned();
        let model = model.clone();
        let favorite = live
            .app
            .preferences
            .favorite_models
            .get(&provider)
            .is_some_and(|models| models.contains(&model));
        content = content.child(
            Button::new(if favorite {
                "Remove favourite model"
            } else {
                "Favourite this model"
            })
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let models = live_mut(s)
                    .app
                    .preferences
                    .favorite_models
                    .entry(provider.clone())
                    .or_default();
                if models.contains(&model) {
                    models.retain(|m| m != &model);
                } else {
                    models.push(model.clone());
                }
            })
            .build(k),
        );
    }
    content
}

fn toggle_menu(s: &mut Workspace, name: &'static str) {
    let menu = &mut live_mut(s).menu;
    *menu = if *menu == Some(name) {
        None
    } else {
        Some(name)
    };
}

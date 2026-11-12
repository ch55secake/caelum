//! Session-first navigation. Feed controls and timers belong to this feature.
mod badge;
mod projects;
mod requests;
mod session;

use std::rc::Rc;

use caelum_core::application::{SessionScope, Workspace as Application};
use rune::{El, Key, KeyEvent, ScrollSpec, Tx, col, icon, row, text, text_input};
use rune_kit::{Kit, ScrollBar, glyphs, shell::SidebarHeader};

use super::{live, live_mut};
use crate::{PLUS, Workspace, context::EditMenu, quiet_button, ui};

#[derive(Default, PartialEq, Eq)]
enum Menu {
    #[default]
    Closed,
    Filter,
}

pub struct State {
    badges: badge::State,
    discovered: std::collections::HashSet<caelum_core::application::Scope>,
    pub project: Option<String>,
    menu: Menu,
    pub now: u64,
    visible: Vec<usize>,
    deadline: Option<u64>,
    query: Option<(Option<String>, String, bool)>,
    request_scope: Option<(SessionScope, Option<String>)>,
    selected: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            badges: Default::default(),
            discovered: Default::default(),
            project: None,
            menu: Menu::Closed,
            now: caelum_core::application::presentation::now(),
            visible: Vec::new(),
            deadline: None,
            query: None,
            request_scope: None,
            selected: None,
        }
    }
}

impl State {
    pub fn discover(&mut self, app: &mut Application) {
        self.discovered
            .retain(|scope| app.project_is_open(&scope.project));
        let scopes: Vec<_> = app
            .projects
            .iter()
            .flat_map(|p| {
                caelum_core::application::Provider::ALL.map(|provider| {
                    caelum_core::application::Scope {
                        project: p.path.clone(),
                        provider,
                    }
                })
            })
            .collect();
        for scope in scopes {
            if self.discovered.insert(scope.clone()) {
                app.discover_sessions(scope);
            }
        }
    }
    pub fn close(&mut self) {
        self.menu = Menu::Closed;
    }
    pub fn hidden(&mut self) {
        self.visible.clear();
        self.deadline = None;
        self.close();
    }
    pub fn poll(
        &mut self,
        app: &mut Application,
        query: &str,
        archived: bool,
        tx: &mut Tx,
        clock: bool,
    ) -> bool {
        let mut changed = self.badges.sync(app);
        if self
            .project
            .as_ref()
            .is_some_and(|p| !app.project_is_open(p))
        {
            self.project = None;
            changed = true;
        }
        let project = self.project.clone().or_else(|| {
            (app.preferences.session_scope == SessionScope::CurrentProject)
                .then(|| {
                    app.active_project
                        .as_ref()
                        .map(|p| app.group_path(p).to_owned())
                })
                .flatten()
        });
        let request_scope = (app.preferences.session_scope, project.clone());
        if self.request_scope.as_ref() != Some(&request_scope) {
            app.refresh_feed_pull_requests(self.project.as_deref());
            self.request_scope = Some(request_scope);
            tx.scroll_to_y("requests-scroll", 0.0);
        } else if self.selected != app.selected
            && let Some(path) = app.active_project.clone()
            && app.feed_includes_project(&path, self.project.as_deref())
        {
            app.refresh_feed_pull_requests(Some(&path));
        }
        let query = (project, query.to_owned(), archived);
        if self.query.as_ref() != Some(&query) {
            let rows = app.session_feed(self.project.as_deref(), &query.1, archived);
            self.visible.retain(|i| rows.contains(i));
            tx.scroll_to_y("sidebar-scroll", 0.0);
            self.query = Some(query);
        }
        if self.selected != app.selected {
            self.menu = Menu::Closed;
            self.selected = app.selected;
            if let Some(index) = app.selected {
                tx.scroll_into_view(format!("live-session-{index}"));
            }
        }
        if clock && !app.window_background {
            let now = caelum_core::application::presentation::now();
            changed |= self
                .visible
                .iter()
                .filter_map(|&i| app.sessions.get(i))
                .any(|s| age(s.last_activity, self.now).0 != age(s.last_activity, now).0);
            self.now = now;
            let deadline = self
                .visible
                .iter()
                .filter_map(|&i| app.sessions.get(i))
                .filter_map(|s| age(s.last_activity, now).1)
                .min();
            if deadline != self.deadline || self.deadline.is_some_and(|at| now >= at) {
                self.deadline = deadline;
                if let Some(at) = deadline {
                    tx.wake_after(std::time::Duration::from_millis(
                        at.saturating_sub(now).max(1),
                    ));
                }
            }
        }
        changed
    }
}

fn age(at: Option<u64>, now: u64) -> (String, Option<u64>) {
    let Some(at) = at else {
        return (String::new(), None);
    };
    let elapsed = now.saturating_sub(at);
    let (unit, suffix) = if elapsed < 3_600_000 {
        (60_000, "m")
    } else if elapsed < 86_400_000 {
        (3_600_000, "h")
    } else {
        (86_400_000, "d")
    };
    let count = elapsed / unit;
    (
        if elapsed < 60_000 {
            "now".into()
        } else {
            format!("{count}{suffix}")
        },
        Some(at.saturating_add((count + 1).saturating_mul(unit))),
    )
}

pub fn sidebar(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let rows = Rc::new(l.app.session_feed(
        l.feed.project.as_deref(),
        &l.panels.sessions.text,
        l.panels.archived,
    ));
    let (keys, builders, visible) = (rows.clone(), rows.clone(), rows.clone());
    let kit = k.clone();
    let sessions = if rows.is_empty() {
        ui::caption(
            if l.app.projects.is_empty() {
                "Open a project to get started"
            } else if !l.panels.sessions.text.is_empty() {
                "No matching sessions"
            } else if l.panels.archived {
                "No archived sessions"
            } else {
                "No sessions in this project"
            },
            k,
        )
        .padding(12.0)
    } else {
        col()
            .w_full()
            .key("sidebar-list")
            .role("list")
            .label("Sessions")
            .focus_group()
            .focus_follows()
            .growth(rune::GrowthSpec::default())
            .virtual_rows(
                rows.len(),
                session::HEIGHT,
                move |_: &Workspace, row| format!("live-session-{}", keys[row]).into(),
                move |s: &Workspace, row| {
                    session::view(s, &kit, builders[row], row, builders.len())
                },
            )
            .on_visible_rows(move |s: &mut Workspace, range| {
                live_mut(s).feed.visible = range.filter_map(|i| visible.get(i).copied()).collect();
                false
            })
    };
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(4.0)
        .label("Projects and sessions")
        .child(ui::brand(s, k))
        .child(projects::view(s, k))
        .child(ui::divider(k))
        .child(
            SidebarHeader::new(if l.panels.archived {
                "Archived sessions"
            } else {
                "Sessions"
            })
            .count(rows.len())
            .action(
                row()
                    .gap(1.0)
                    .child(
                        quiet_button(&PLUS, "New session · ⌘N")
                            .on_press(|s: &mut Workspace| s.new_session())
                            .build(k)
                            .key("new-session"),
                    )
                    .child(
                        quiet_button(
                            &ui::ARCHIVE,
                            if l.panels.archived {
                                "Show active sessions"
                            } else {
                                "Show archived sessions"
                            },
                        )
                        .expanded(l.panels.archived)
                        .on_press(|s: &mut Workspace| {
                            let p = &mut live_mut(s).panels;
                            p.archived = !p.archived;
                        })
                        .build(k)
                        .key("toggle-archived"),
                    )
                    .child(
                        quiet_button(&ui::REFRESH, "Refresh sessions")
                            .disabled(l.app.projects.is_empty())
                            .on_press(|s: &mut Workspace| {
                                let l = live_mut(s);
                                l.app.refresh_project();
                                l.app.refresh_feed_pull_requests(l.feed.project.as_deref());
                                l.app.discover_open_sessions();
                                l.app.refresh_open_sessions();
                            })
                            .build(k)
                            .key("refresh-sessions"),
                    ),
            )
            .build(k)
            .key("sessions-heading"),
        )
        .child(projects::filter(s).trigger(k).w_full())
        .child(
            row()
                .key("session-search-field")
                .w_full()
                .h(32.0)
                .px(10.0)
                .gap(7.0)
                .center_y()
                .radius(6.0)
                .fill(k.c.bg_base)
                .border(
                    1.0,
                    if l.panels.sessions.focused {
                        k.c.border_focus
                    } else {
                        k.c.border_muted
                    },
                )
                .child(
                    icon(&glyphs::SEARCH, 13.0)
                        .color(k.c.icon_muted)
                        .a11y_hidden(),
                )
                .child(
                    text_input(&l.panels.sessions, k.ty.ui)
                        .single_line()
                        .label("Search sessions")
                        .placeholder("Search sessions", k.c.text_muted)
                        .color(k.c.text_base)
                        .selection_color(k.selection())
                        .grow(1.0)
                        .min_w(0.0)
                        .h(30.0)
                        .py(5.0)
                        .edit_menu("session-search", |s| Some(&mut live_mut(s).panels.sessions))
                        .on_input_key(on_key),
                )
                .child(
                    text("⌘K")
                        .font(k.ty.small)
                        .color(k.c.text_faint)
                        .a11y_hidden(),
                ),
        )
        .child(
            col()
                .key("sidebar-scroll")
                .w_full()
                .grow(1.0)
                .min_h(80.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(sessions)
                .scroll_bar(k),
        )
        .child(requests::view(s, k))
}

pub(super) fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    if live(s).panels.overlay.is_some() {
        return;
    }
    match live(s).feed.menu {
        Menu::Filter => {
            projects::filter(s).build(k, layers);
        }
        Menu::Closed => {}
    }
}

pub(super) fn select(s: &mut Workspace, index: usize) {
    let l = live_mut(s);
    let group = l
        .app
        .group_path(&l.app.sessions[index].scope.project)
        .to_owned();
    l.app.select(index);
    l.app
        .preferences
        .worktree_expanded
        .entry(group)
        .or_insert(false);
    l.panels.screen = crate::panels::Screen::Conversation;
    l.menu = None;
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    let l = live(s);
    if l.feed.menu != Menu::Closed {
        if ev.key == Key::Escape {
            live_mut(s).feed.menu = Menu::Closed;
            return true;
        }
        return false;
    }
    if !l.panels.sessions.text.is_empty() && ev.key == Key::Escape {
        live_mut(s).panels.sessions.clear();
        return true;
    }
    false
}

#[cfg(test)]
mod tests;

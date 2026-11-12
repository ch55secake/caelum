use std::collections::HashSet;

use rune::{El, ScrollSpec, col, icon, row, text, text_input};
use rune_kit::shell::{RowStatus, SidebarHeader, SidebarRow};
use rune_kit::{ButtonVariant, Kit, ScrollBar, glyphs};

use crate::context::EditMenu;
use crate::ui::Button;
use crate::{PLUS, Workspace, quiet_button, ui};

use super::{live, live_mut, open_project};

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

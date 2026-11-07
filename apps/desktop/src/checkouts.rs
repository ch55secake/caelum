//! Project worktrees stay visible in navigation, with independent session ownership.
use std::collections::HashSet;

use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    quiet_button, ui,
};
use caelum_core::application::{Task, Workspace as Application};
use caelum_protocol::GitWorktree;
use rune::{El, col, icon, row, text};
use rune_kit::{Kit, glyphs};

#[derive(Default)]
pub struct State {
    requested: HashSet<String>,
    pub project: Option<String>,
}

impl State {
    pub fn sync(&mut self, app: &mut Application) -> bool {
        self.requested.retain(|path| app.project_is_open(path));
        let roots: Vec<_> = app
            .projects
            .iter()
            .filter(|p| app.group_path(&p.path) == p.path)
            .map(|p| p.path.clone())
            .collect();
        let mut changed = false;
        for project in roots {
            if !self.requested.insert(project.clone()) {
                continue;
            }
            let task = refresh_task(app, &project);
            if app.services.worktrees(&project).is_none()
                && !app.services.busy(&task)
                && app.services.error(&task).is_none()
            {
                app.task(task);
                changed = true;
            }
        }
        changed
    }
}

fn refresh_task(app: &Application, project: &str) -> Task {
    Task::Worktrees {
        project: project.into(),
        directory: app.preferences.worktree_directory.clone(),
    }
}

pub fn manage(s: &mut Workspace, project: String) {
    let l = live_mut(s);
    let project = l.app.group_path(&project).to_owned();
    l.app.task(refresh_task(&l.app, &project));
    l.checkouts.project = Some(project);
    l.panels.overlay = Some(Screen::Worktrees);
}

pub fn select(s: &mut Workspace, path: &str) {
    let l = live_mut(s);
    if !l.app.select_checkout(path) {
        return;
    }
    l.sync_editors();
    l.panels.screen = Screen::Conversation;
    l.panels.overlay = None;
    l.panels.archived = false;
    l.session_menu = None;
    l.app.task(Task::Git {
        project: path.into(),
    });
    l.app.refresh_files();
}

pub fn status(app: &Application, tree: &GitWorktree) -> String {
    if !tree.available {
        return "Missing checkout".into();
    }
    let path = &tree.project.path;
    let attention = app
        .approvals
        .iter()
        .filter(|p| &p.project_path == path)
        .count()
        + app
            .questions
            .iter()
            .filter(|p| &p.project_path == path)
            .count();
    let sessions: Vec<_> = app
        .sessions
        .iter()
        .filter(|s| &s.scope.project == path && !s.archived && !s.forgotten)
        .collect();
    let working = sessions
        .iter()
        .filter(|s| s.working() || !s.shells.is_empty())
        .count();
    let queued: usize = sessions
        .iter()
        .map(|s| s.queue.len() + s.inbox.items.len())
        .sum();
    let detail = if attention > 0 {
        format!("{attention} need attention")
    } else if working > 0 {
        format!("{working} working")
    } else if queued > 0 {
        format!("{queued} queued")
    } else if sessions.iter().any(|s| s.unread) {
        "Unread activity".into()
    } else if tree.locked.is_some() {
        "Locked".into()
    } else if sessions.is_empty() {
        "No sessions yet".into()
    } else {
        format!(
            "{} session{}",
            sessions.len(),
            if sessions.len() == 1 { "" } else { "s" }
        )
    };
    if tree.main {
        format!("Main checkout · {detail}")
    } else {
        detail
    }
}

pub fn sidebar(s: &Workspace, k: &Kit, project: &str) -> Option<(El, f32)> {
    let app = &live(s).app;
    let catalog = app.services.worktrees(project);
    let task = refresh_task(app, project);
    let busy = app.services.busy(&task);
    let error = app.services.error(&task);
    if catalog.is_none() && !busy && error.is_none() {
        return None;
    }
    let count = catalog.map_or(0, |listing| {
        listing.worktrees.iter().filter(|tree| !tree.main).count()
    });
    let active = app
        .active_project
        .as_ref()
        .is_some_and(|p| app.group_path(p) == project);
    let expanded = app
        .preferences
        .worktree_expanded
        .get(project)
        .copied()
        .unwrap_or(active && count > 0);
    let toggle_project = project.to_owned();
    let manage_project = project.to_owned();
    let mut body = col().w_full().gap(2.0).pl(8.0).child(
        row()
            .w_full()
            .h(28.0)
            .center_y()
            .gap(2.0)
            .child(
                k.interactive(
                    row()
                        .h_full()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(6.0)
                        .center_y()
                        .radius(5.0)
                        .px(4.0)
                        .child(
                            icon(
                                if expanded {
                                    &glyphs::CHEVRON_DOWN
                                } else {
                                    &glyphs::CARET_RIGHT
                                },
                                12.0,
                            )
                            .color(k.c.icon_muted),
                        )
                        .child(text("Worktrees").font(k.ty.small).color(k.c.text_muted))
                        .child(
                            text(count.to_string())
                                .font(k.ty.small)
                                .color(k.c.text_faint),
                        ),
                )
                .key(crate::code_view::source_key(project, "worktrees-toggle"))
                .role("button")
                .label(if expanded {
                    "Hide worktrees"
                } else {
                    "Show worktrees"
                })
                .description(project)
                .expanded(expanded)
                .focusable(true)
                .focus_ring(1.0, 0.0, k.c.border_focus)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s)
                        .app
                        .preferences
                        .worktree_expanded
                        .insert(toggle_project.clone(), !expanded);
                }),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh worktrees")
                    .disabled(busy)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
                    .build(k)
                    .size(22.0, 24.0)
                    .key(crate::code_view::source_key(project, "refresh-worktrees")),
            )
            .child(
                quiet_button(&ui::MORE, "Manage worktrees")
                    .on_press(move |s: &mut Workspace| manage(s, manage_project.clone()))
                    .build(k)
                    .size(22.0, 24.0)
                    .key(crate::code_view::source_key(project, "manage-worktrees")),
            ),
    );
    let mut height = 30.0;
    if expanded {
        if let Some(catalog) = catalog {
            for tree in &catalog.worktrees {
                let path = tree.project.path.clone();
                let selected = app.active_project.as_ref() == Some(&path);
                let branch = tree.branch.as_deref().unwrap_or("Detached HEAD");
                let status = status(app, tree);
                let mut help = format!("{branch}\n{path}\n{status}");
                if let Some(reason) = &tree.locked {
                    help.push_str(&format!("\nLocked: {reason}"));
                }
                body = body.child(
                    k.interactive(
                        row()
                            .w_full()
                            .h(42.0)
                            .px(8.0)
                            .gap(7.0)
                            .center_y()
                            .radius(6.0)
                            .fill(if selected {
                                k.c.hover
                            } else {
                                rune::Color::TRANSPARENT
                            })
                            .child(icon(&ui::BRANCH, 12.0).color(if selected {
                                k.c.icon_accent
                            } else {
                                k.c.icon_muted
                            }))
                            .child(
                                col()
                                    .grow(1.0)
                                    .min_w(0.0)
                                    .gap(2.0)
                                    .child(
                                        text(branch).font(k.ty.ui).color(k.c.text_base).truncate(),
                                    )
                                    .child(
                                        text(&status)
                                            .font(rune::TextStyle::new(10.0, 400.0))
                                            .color(k.c.text_muted)
                                            .truncate(),
                                    ),
                            )
                            .children(
                                selected.then(|| {
                                    icon(k.glyph.check_small, 12.0).color(k.c.icon_accent)
                                }),
                            ),
                    )
                    .key(crate::code_view::source_key(&tree.project.path, "checkout"))
                    .role("button")
                    .label(format!("Switch to worktree {branch}"))
                    .description(format!(
                        "{status} · {}{}",
                        tree.project.path,
                        if selected { " · Current checkout" } else { "" }
                    ))
                    .help(help)
                    .focusable(true)
                    .focus_selected(selected)
                    .focus_ring(1.0, 0.0, k.c.border_focus)
                    .disabled(!tree.available)
                    .opacity(if tree.available { 1.0 } else { 0.5 })
                    .on_press(move |s: &mut Workspace| select(s, &path)),
                );
                height += 44.0;
            }
        } else if busy {
            body = body.child(ui::caption("Loading worktrees…", k).padding(8.0));
            height += 34.0;
        }
    }
    if let Some(error) = error {
        body = body.child(
            ui::caption("Worktrees unavailable", k)
                .color(k.c.danger.fg)
                .truncate()
                .help(error)
                .px(8.0),
        );
        height += 20.0;
    }
    Some((body, height))
}

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
use rune::{El, ScrollSpec, col, icon, row, text};
use rune_kit::{ButtonVariant, Kit, ScrollBar, glyphs};

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

pub fn name(tree: &GitWorktree) -> &str {
    std::path::Path::new(&tree.path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&tree.path)
}

pub fn project_name<'a>(app: &'a Application, project: &'a str) -> &'a str {
    app.projects
        .iter()
        .find(|p| p.path == project)
        .map(|p| p.name.as_str())
        .unwrap_or_else(|| project.rsplit('/').next().unwrap_or(project))
}

fn owner(s: &Workspace) -> Option<&str> {
    let l = live(s);
    l.checkouts
        .project
        .as_deref()
        .or(l.app.active_project.as_deref())
}

pub fn manager_height(s: &Workspace) -> f32 {
    let app = &live(s).app;
    let count = owner(s).map_or(0, |project| app.managed_worktrees(project).count());
    let error = owner(s).is_some_and(|project| manager_error(app, project).is_some());
    (132.0 + count as f32 * 65.0 + if error { 96.0 } else { 0.0 }).clamp(240.0, 400.0)
}

fn manager_error<'a>(app: &'a Application, project: &str) -> Option<&'a str> {
    app.services
        .error(&Task::CreateWorktree {
            project: project.into(),
            branch: String::new(),
            base: String::new(),
            directory: String::new(),
        })
        .or_else(|| app.services.error(&refresh_task(app, project)))
        .or_else(|| {
            app.services
                .checkouts
                .get(project)
                .and_then(|c| c.error.as_deref())
        })
}

pub fn manager(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let Some(project) = owner(s) else {
        return ui::empty(
            "Open a project",
            "Choose a project to create a Caelum worktree.",
            k,
        );
    };
    let parent = project_name(app, project);
    let refresh = refresh_task(app, project);
    let mutation = Task::CreateWorktree {
        project: project.into(),
        branch: String::new(),
        base: String::new(),
        directory: String::new(),
    };
    let busy = app.services.busy(&mutation);
    let mut body = col().w_full().h_full().min_h(0.0).gap(12.0).child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .child(icon(&ui::FOLDER, 20.0).color(k.c.icon_muted))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(3.0)
                    .child(text(parent).font(k.ty.ui_medium).color(k.c.text_base))
                    .child(ui::caption(project, k).truncate().help(project)),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh worktrees")
                    .disabled(app.services.busy(&refresh))
                    .on_press({
                        let refresh = refresh.clone();
                        move |s: &mut Workspace| live_mut(s).app.task(refresh.clone())
                    })
                    .build(k)
                    .key("manager-refresh-worktrees"),
            ),
    );
    if let Some(error) = manager_error(app, project) {
        body = body.child(ui::notice("Couldn't update worktrees", error, k));
    }
    let mut rows = col()
        .w_full()
        .focus_group()
        .role("list")
        .label(format!("Caelum worktrees for {parent}"));
    let trees: Vec<_> = app.managed_worktrees(project).collect();
    let count = trees.len();
    for (index, tree) in trees.into_iter().enumerate() {
        if index > 0 {
            rows = rows.child(ui::divider(k));
        }
        let current = app.active_project.as_ref() == Some(&tree.project.path);
        let path = tree.project.path.clone();
        let detail = format!(
            "{} · {}",
            tree.branch.as_deref().unwrap_or("Detached HEAD"),
            status(app, tree)
        );
        rows = rows.child(
            row()
                .w_full()
                .gap(4.0)
                .center_y()
                .py(3.0)
                .child(
                    ui::choice(name(tree), &detail, current, k)
                        .grow(1.0)
                        .min_w(0.0)
                        .h(58.0)
                        .disabled(!tree.available || busy)
                        .label(format!("Open worktree {} for {parent}", name(tree)))
                        .help(format!("{}\n{}\n{detail}", parent, tree.project.path))
                        .key(crate::code_view::source_key(
                            &tree.project.path,
                            "manage-checkout",
                        ))
                        .on_press(move |s: &mut Workspace| select(s, &path)),
                )
                .child(
                    ui::Button::new("Remove")
                        .variant(ButtonVariant::GhostMuted)
                        .disabled(busy || tree.locked.is_some())
                        .on_press({
                            let task = Task::RemoveWorktree {
                                project: project.into(),
                                path: tree.path.clone(),
                            };
                            move |s: &mut Workspace| live_mut(s).app.task(task.clone())
                        })
                        .build(k)
                        .px(8.0)
                        .label(format!("Remove worktree {} from {parent}", name(tree)))
                        .help(
                            tree.locked
                                .as_deref()
                                .unwrap_or("Remove worktree; keep its branch and commits"),
                        )
                        .key(crate::code_view::source_key(
                            &tree.project.path,
                            "remove-worktree",
                        )),
                ),
        );
    }
    if count == 0 {
        rows = rows.child(
            if app.services.worktrees(project).is_none() && app.services.busy(&refresh) {
                ui::skeleton("Loading Caelum worktrees", 2, k)
            } else {
                col()
                    .w_full()
                    .py(20.0)
                    .gap(8.0)
                    .child(
                        row()
                            .center_y()
                            .gap(8.0)
                            .child(icon(&ui::FOLDER, 18.0).color(k.c.icon_muted))
                            .child(
                                text("No Caelum worktrees yet")
                                    .font(k.ty.ui_medium)
                                    .color(k.c.text_base),
                            ),
                    )
                    .child(
                        ui::caption(
                            format!("Create a worktree to work on {parent} in parallel."),
                            k,
                        )
                        .wrap(),
                    )
            },
        );
    }
    let provider = app
        .selected
        .map(|i| &app.sessions[i])
        .filter(|session| app.group_path(&session.scope.project) == project)
        .map_or(app.preferences.default_provider, |session| {
            session.scope.provider
        });
    body.child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .key("worktrees-scroll")
            .child(rows)
            .scroll_bar(k),
    )
    .child(ui::divider(k))
    .child(
        row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                ui::Button::new("Open project folder")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press({
                        let path = project.to_owned();
                        move |s: &mut Workspace| select(s, &path)
                    })
                    .build(k)
                    .key(crate::code_view::source_key(project, "manage-checkout")),
            )
            .child(rune::el().grow(1.0))
            .child(
                ui::Button::new("New worktree")
                    .icon(&crate::PLUS)
                    .variant(ButtonVariant::Contrast)
                    .disabled(busy)
                    .on_press({
                        let path = project.to_owned();
                        move |s: &mut Workspace| crate::worktree::open(s, path.clone(), provider)
                    })
                    .build(k)
                    .key("manager-new-worktree"),
            ),
    )
}

pub fn sidebar(s: &Workspace, k: &Kit, project: &str) -> Option<(El, f32)> {
    let app = &live(s).app;
    let count = app.managed_worktrees(project).count();
    if count == 0 {
        return None;
    }
    let task = refresh_task(app, project);
    let busy = app.services.busy(&task);
    let error = app.services.error(&task);
    let parent = project_name(app, project);
    let active = app
        .active_project
        .as_ref()
        .is_some_and(|p| app.group_path(p) == project);
    let expanded = app
        .preferences
        .worktree_expanded
        .get(project)
        .copied()
        .unwrap_or(active);
    let toggle_project = project.to_owned();
    let manage_project = project.to_owned();
    let mut body = col()
        .w_full()
        .gap(2.0)
        .pl(16.0)
        .pr(6.0)
        .pb(6.0)
        .role("group")
        .label(format!("Caelum worktrees for {parent}"))
        .key(crate::code_view::source_key(project, "worktree-group"))
        .child(
            rune::el()
                .w(1.0)
                .fill(k.c.border_muted)
                .pin(Some(9.0), Some(5.0), None, Some(12.0)),
        )
        .child(
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
                            .child(
                                text("Caelum worktrees")
                                    .font(k.ty.small)
                                    .color(k.c.text_muted)
                                    .truncate(),
                            )
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
                    .description(format!("Worktrees belonging to {parent} · {project}"))
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
    let mut height = 36.0;
    if expanded {
        for tree in app.managed_worktrees(project) {
            let path = tree.project.path.clone();
            let selected = app.active_project.as_ref() == Some(&path);
            let branch = tree.branch.as_deref().unwrap_or("Detached HEAD");
            let status = status(app, tree);
            let mut help = format!("{parent} / {}\n{branch}\n{path}\n{status}", name(tree));
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
                        .child(icon(&ui::FOLDER, 12.0).color(if selected {
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
                                    text(name(tree))
                                        .font(k.ty.ui)
                                        .color(k.c.text_base)
                                        .truncate(),
                                )
                                .child(
                                    text(&status)
                                        .font(rune::TextStyle::new(10.0, 400.0))
                                        .color(k.c.text_muted)
                                        .truncate(),
                                ),
                        )
                        .children(
                            selected
                                .then(|| icon(k.glyph.check_small, 12.0).color(k.c.icon_accent)),
                        ),
                )
                .key(crate::code_view::source_key(&tree.project.path, "checkout"))
                .role("button")
                .label(format!("Switch to worktree {}", name(tree)))
                .description(format!(
                    "{parent} · {status} · {}{}",
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

use rune::{El, col, row};
use rune_kit::{Kit, ScrollBar};

use crate::context::EditMenu;
use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, ui};

use super::{Screen, scroll};

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

pub(super) fn launcher(s: &Workspace, k: &Kit) -> El {
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
            "Find a project or worktree…",
            k,
        )
        .edit_menu("launch-search", |s| Some(&mut live_mut(s).commands.search)),
    );
    let query = l.commands.search.text.to_lowercase();
    let mut projects = col().w_full().gap(12.0);
    for project in l
        .app
        .projects
        .iter()
        .filter(|p| l.app.group_path(&p.path) == p.path)
    {
        let matches_project = format!("{} {}", project.name, project.path)
            .to_lowercase()
            .contains(&query);
        let trees: Vec<_> = l
            .app
            .managed_worktrees(&project.path)
            .filter(|tree| {
                matches_project
                    || format!(
                        "{} {}",
                        tree.path,
                        tree.branch.as_deref().unwrap_or_default()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        if !matches_project && trees.is_empty() {
            continue;
        }
        let path = project.path.clone();
        let tree_project = path.clone();
        let manage_project = path.clone();
        let mut group = col()
            .w_full()
            .gap(2.0)
            .role("group")
            .label(format!("{} project and worktrees", project.name))
            .key(crate::code_view::source_key(&path, "launch-group"))
            .child(
                row()
                    .w_full()
                    .gap(6.0)
                    .center_y()
                    .child(
                        ui::choice(
                            &project.name,
                            &format!("Project folder · {}", project.path),
                            l.app.active_project.as_ref() == Some(&path),
                            k,
                        )
                        .on_press(move |s: &mut Workspace| launch(s, &path))
                        .h(50.0)
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
                            .label(format!("Create worktree for {}", project.name))
                            .key(crate::code_view::source_key(
                                &project.path,
                                "launch-worktree",
                            )),
                    )
                    .child(
                        quiet_button(&ui::MORE, "Manage this project's worktrees")
                            .on_press(move |s: &mut Workspace| {
                                crate::checkouts::manage(s, manage_project.clone())
                            })
                            .build(k)
                            .key(crate::code_view::source_key(
                                &project.path,
                                "launch-manage-worktrees",
                            )),
                    ),
            );
        if !trees.is_empty() {
            let mut children = col()
                .w_full()
                .pl(24.0)
                .gap(2.0)
                .child(ui::caption("Worktrees", k).px(10.0).py(3.0));
            for tree in trees {
                let path = tree.project.path.clone();
                children = children.child(crate::context::worktree(
                    ui::choice(
                        crate::checkouts::name(tree),
                        tree.branch.as_deref().unwrap_or("Detached HEAD"),
                        l.app.active_project.as_ref() == Some(&path),
                        k,
                    )
                    .disabled(!tree.available)
                    .h(46.0)
                    .label(format!(
                        "New session in {} / {}",
                        project.name,
                        crate::checkouts::name(tree)
                    ))
                    .help(&tree.project.path)
                    .on_press(move |s: &mut Workspace| launch(s, &path))
                    .key(crate::code_view::source_key(&tree.project.path, "launch")),
                    project.path.clone(),
                    tree.path.clone(),
                    crate::code_view::source_key(&tree.project.path, "launch"),
                ));
            }
            group = group.child(children);
        }
        if let Some(error) = crate::checkouts::error(&l.app, &project.path) {
            group = group.child(ui::caption(error, k).wrap().color(k.c.danger.fg).px(10.0));
        }
        projects = projects.child(group);
    }
    body.child(
        scroll(projects)
            .key("launcher-projects-scroll")
            .scroll_bar(k),
    )
    .child(ui::divider(k))
    .child(
        row().gap(8.0).child(
            Button::new("Open folder…")
                .on_press_tx(crate::live::open_project)
                .build(k),
        ),
    )
}

fn launch(s: &mut Workspace, path: &str) {
    let l = live_mut(s);
    if !l.app.select_checkout(path) {
        return;
    }
    l.app.project_draft(caelum_core::application::Scope {
        project: path.into(),
        provider: l.panels.launch_provider,
    });
    l.sync_editors();
    l.panels.overlay = None;
    l.panels.screen = Screen::Conversation;
    l.focus();
}

use super::{Menu, live, live_mut};
use crate::{
    PLUS, Workspace, quiet_button,
    select::{Choice, Select},
    ui,
};
use rune::{El, ScrollSpec, col, row, text};
use rune_kit::{Kit, ScrollBar, glyphs, shell::SidebarHeader};

pub(super) fn filter(s: &Workspace) -> Select {
    let l = live(s);
    let default = match l.app.preferences.session_scope {
        caelum_core::application::SessionScope::AllProjects => "All projects".to_owned(),
        caelum_core::application::SessionScope::CurrentProject => l
            .app
            .active_project
            .as_ref()
            .map_or("Current project".into(), |p| {
                format!(
                    "Current · {}",
                    crate::checkouts::project_name(&l.app, l.app.group_path(p))
                )
            }),
    };
    let choices = std::iter::once(Choice::new("", default))
        .chain(
            l.app
                .projects
                .iter()
                .filter(|p| l.app.group_path(&p.path) == p.path)
                .map(|p| Choice::new(&p.path, &p.name)),
        )
        .collect();
    Select::choices("session-project-filter", choices)
        .label("Filter sessions and pull requests by project")
        .current(Some(l.feed.project.as_deref().unwrap_or("")))
        .open(l.feed.menu == Menu::Filter)
        .on_toggle(|s: &mut Workspace| {
            let l = live_mut(s);
            l.feed.menu = if l.feed.menu == Menu::Filter {
                Menu::Closed
            } else {
                Menu::Filter
            };
        })
        .on_close(|s: &mut Workspace| live_mut(s).feed.menu = Menu::Closed)
        .on_choose(|s: &mut Workspace, value, _| {
            let l = live_mut(s);
            l.feed.project = (!value.is_empty()).then(|| value.to_owned());
            l.feed.menu = Menu::Closed;
        })
}

pub(super) fn view(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let roots: Vec<_> = app
        .projects
        .iter()
        .filter(|p| app.group_path(&p.path) == p.path)
        .collect();
    let mut projects = col()
        .key("projects-list")
        .role("list")
        .w_full()
        .gap(4.0)
        .focus_group();
    let mut height = (roots.len() as f32 * 36.0 - 4.0).max(32.0);
    for project in &roots {
        let path = project.path.clone();
        let selected = app
            .active_project
            .as_ref()
            .is_some_and(|p| app.group_path(p) == path);
        let create_key = crate::code_view::source_key(&path, "create-worktree");
        let close_key = crate::code_view::source_key(&path, "close-project");
        let mut group = col()
            .w_full()
            .key(crate::code_view::source_key(&path, "project-group"))
            .role("group")
            .label(format!("{} project and worktrees", project.name))
            .child(crate::context::project(
                k.interactive(
                    row()
                        .w_full()
                        .h(32.0)
                        .px(8.0)
                        .gap(7.0)
                        .center_y()
                        .radius(6.0)
                        .fill(if selected {
                            k.c.hover
                        } else {
                            rune::Color::TRANSPARENT
                        })
                        .child(super::badge::view(app, &path, k))
                        .child(
                            text(&project.name)
                                .font(k.ty.ui_medium)
                                .color(k.c.text_base)
                                .truncate()
                                .grow(1.0)
                                .min_w(0.0),
                        )
                        .child(
                            quiet_button(&ui::BRANCH, "Create worktree")
                                .on_press({
                                    let path = path.clone();
                                    move |s: &mut Workspace| {
                                        live_mut(s).feed.menu = Menu::Closed;
                                        crate::worktree::open(
                                            s,
                                            path.clone(),
                                            live(s).app.preferences.default_provider,
                                        );
                                    }
                                })
                                .build(k)
                                .size(22.0, 24.0)
                                .key(create_key),
                        )
                        .child(
                            quiet_button(&glyphs::CLOSE, "Close project")
                                .on_press({
                                    let path = path.clone();
                                    move |s: &mut Workspace| live_mut(s).close_project(&path)
                                })
                                .build(k)
                                .size(22.0, 24.0)
                                .key(close_key)
                                .label(format!("Close {}", project.name)),
                        ),
                )
                .role("button")
                .label(&project.name)
                .description(&path)
                .help(&path)
                .focus_group()
                .focusable(true)
                .focus_selected(selected)
                .focus_return(if roots.len() == 1 {
                    "open-project"
                } else {
                    "projects-list"
                })
                .focus_ring(1.0, 0.0, k.c.border_focus)
                .key(crate::code_view::source_key(&path, "project"))
                .on_press({
                    let path = path.clone();
                    move |s: &mut Workspace| {
                        live_mut(s).feed.menu = Menu::Closed;
                        crate::checkouts::select(s, &path);
                    }
                }),
                path.clone(),
            ));
        if let Some((trees, tree_height)) = crate::checkouts::sidebar(s, k, &path) {
            group = group.child(trees);
            height += tree_height;
        }
        projects = projects.child(group);
    }
    col()
        .key("projects-section")
        .w_full()
        .grow(0.65)
        .min_h((height + 32.0).min(100.0))
        .max_h((height + 32.0).min(280.0))
        .gap(4.0)
        .child(
            SidebarHeader::new(if app.opening {
                "Opening project…"
            } else {
                "Projects"
            })
            .count(roots.len())
            .action(
                quiet_button(&PLUS, "Open project")
                    .on_press_tx(|s: &mut Workspace, tx| {
                        live_mut(s).feed.menu = Menu::Closed;
                        super::super::open_project(s, tx);
                    })
                    .build(k)
                    .key("open-project"),
            )
            .build(k)
            .key("projects-heading"),
        )
        .child(
            col()
                .key("projects-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
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
}

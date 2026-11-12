use super::{Menu, live, live_mut};
use crate::{
    PLUS, Workspace, quiet_button,
    select::{Choice, Select},
    ui,
};
use rune::{El, ScrollSpec, col, icon, row, text};
use rune_kit::{Kit, Placement, ScrollBar, glyphs, shell::SidebarHeader};

pub(super) fn trigger(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let path = app.active_project.as_deref();
    let name = path
        .map(|p| crate::checkouts::project_name(app, app.group_path(p)))
        .unwrap_or("Choose project");
    let display = path.filter(|p| app.group_path(p) != *p).map_or_else(
        || name.to_owned(),
        |p| format!("{name} / {}", crate::checkouts::project_name(app, p)),
    );
    Select::new::<&str>("project-picker", &[])
        .display(if app.opening {
            "Opening project…"
        } else {
            &display
        })
        .label("Projects and worktrees")
        .icon(&ui::FOLDER)
        .prominent()
        .open(live(s).feed.menu == Menu::Projects)
        .on_toggle(|s: &mut Workspace| {
            let l = live_mut(s);
            l.feed.menu = if l.feed.menu == Menu::Projects {
                Menu::Closed
            } else {
                Menu::Projects
            };
        })
        .trigger(k)
        .w_full()
        .help(path.unwrap_or("Open or choose a project"))
}

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
        .label("Filter sessions by project")
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

pub(super) fn overlay(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let app = &live(s).app;
    let mut projects = col()
        .key("projects-list")
        .role("list")
        .w_full()
        .gap(4.0)
        .focus_group();
    for project in app
        .projects
        .iter()
        .filter(|p| app.group_path(&p.path) == p.path)
    {
        let path = project.path.clone();
        let selected = app
            .active_project
            .as_ref()
            .is_some_and(|p| app.group_path(p) == path);
        let mut group = col()
            .w_full()
            .key(crate::code_view::source_key(&path, "project-group"))
            .role("group")
            .label(format!("{} project and worktrees", project.name))
            .child(crate::context::project(
                k.interactive(
                    row()
                        .w_full()
                        .h(46.0)
                        .px(8.0)
                        .gap(7.0)
                        .center_y()
                        .radius(6.0)
                        .child(icon(&ui::FOLDER, 14.0).color(if selected {
                            k.c.icon_accent
                        } else {
                            k.c.icon_muted
                        }))
                        .child(
                            col()
                                .min_w(0.0)
                                .grow(1.0)
                                .child(
                                    text(&project.name)
                                        .font(k.ty.ui_medium)
                                        .color(k.c.text_base)
                                        .truncate(),
                                )
                                .child(ui::caption("Project folder", k)),
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
                                .key(crate::code_view::source_key(&path, "create-worktree")),
                        )
                        .child(
                            quiet_button(&glyphs::CLOSE, "Close project")
                                .on_press({
                                    let path = path.clone();
                                    move |s: &mut Workspace| live_mut(s).close_project(&path)
                                })
                                .build(k)
                                .size(22.0, 24.0)
                                .key(crate::code_view::source_key(&path, "close-project"))
                                .label(format!("Close {}", project.name)),
                        ),
                )
                .role("button")
                .label(&project.name)
                .description(&path)
                .help(&path)
                .focus_group()
                .focusable(true)
                .focus_return("project-picker-trigger")
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
        if let Some((trees, _)) = crate::checkouts::sidebar(s, k, &path) {
            group = group.child(trees);
        }
        projects = projects.child(group);
    }
    let content = col()
        .w(340.0)
        .gap(4.0)
        .child(
            SidebarHeader::new("Projects")
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
                .max_h(350.0)
                .min_h(40.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(projects)
                .scroll_bar(k),
        );
    ui::popup(
        "project-picker",
        "project-picker-trigger",
        content,
        Placement::BottomStart,
        |s: &mut Workspace| live_mut(s).feed.menu = Menu::Closed,
        k,
        layers,
    );
}

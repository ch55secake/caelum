use super::{Menu, close_menu, toggle_menu};
use crate::{
    Workspace,
    live::{live, live_mut},
    select::{Choice, Select},
    ui,
};
use caelum_core::application::Provider;
use rune::{El, FileDialogOptions, Tx, col, row};
use rune_kit::{ButtonVariant, Kit};

pub(super) fn project(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let project = l.app.home.draft.project.as_deref();
    let name = project
        .and_then(|p| l.app.projects.iter().find(|v| v.path == p))
        .map(|p| p.name.as_str())
        .unwrap_or("Choose a project");
    let expanded = l.home.menu == Menu::Projects || !l.app.home_project_available();
    let heading = row()
        .w_full()
        .center_y()
        .gap(6.0)
        .child(
            ui::Button::new(name)
                .icon(&ui::FOLDER)
                .variant(ButtonVariant::GhostMuted)
                .on_press(|s| toggle_menu(s, Menu::Projects))
                .build(k)
                .key("home-project")
                .label("Project or worktree")
                .help(project.unwrap_or("Choose where the new session will run")),
        )
        .children(
            project
                .and_then(|p| l.app.services.checkouts.get(p))
                .and_then(|c| c.git.as_ref())
                .and_then(|g| g.branch.as_deref())
                .map(|branch| ui::caption(branch, k).truncate()),
        );
    let mut content = col().w_full().gap(6.0).child(heading);
    if expanded {
        let mut choices = col()
            .key("home-project-list")
            .role("list")
            .label("Projects and worktrees")
            .focus_group()
            .focus_follows()
            .growth(rune::GrowthSpec::default())
            .w_full()
            .gap(4.0);
        let paths: std::rc::Rc<Vec<_>> =
            std::rc::Rc::new(l.app.projects.iter().map(|p| p.path.clone()).collect());
        let keys = paths.clone();
        let kit = k.clone();
        choices = choices.virtual_rows(
            paths.len(),
            48.0,
            move |_: &Workspace, i| format!("home-project-{}", keys[i]).into(),
            move |s: &Workspace, i| {
                let path = &paths[i];
                let app = &live(s).app;
                let title = app
                    .projects
                    .iter()
                    .find(|p| &p.path == path)
                    .map(|p| p.name.as_str())
                    .unwrap_or(path);
                let choose = path.clone();
                let tree = app
                    .services
                    .worktrees(app.group_path(path))
                    .and_then(|listing| listing.worktrees.iter().find(|t| &t.project.path == path));
                let available = tree.is_none_or(|t| t.available);
                let detail = tree
                    .and_then(|t| t.branch.as_ref())
                    .map_or_else(|| path.clone(), |branch| format!("{branch} · {path}"));
                ui::choice(
                    title,
                    &detail,
                    app.home.draft.project.as_ref() == Some(path),
                    &kit,
                )
                .disabled(!available)
                .focusable(available)
                .on_press(move |s: &mut Workspace| {
                    let l = live_mut(s);
                    l.app.home_select_project(choose.clone());
                    l.home.menu = Menu::Closed;
                    l.focus();
                })
            },
        );
        content = content.child(
            col()
                .w_full()
                .padding(10.0)
                .gap(6.0)
                .radius(10.0)
                .fill(k.c.layer_01)
                .border(1.0, k.c.border_base)
                .children((!l.app.projects.is_empty()).then(|| {
                    col()
                        .w_full()
                        .max_h(170.0)
                        .scroll(rune::ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(choices)
                }))
                .child(
                    ui::Button::new("Open project…")
                        .icon(&ui::FOLDER)
                        .variant(ButtonVariant::GhostMuted)
                        .disabled(l.app.home.opening())
                        .on_press_tx(open_project)
                        .build(k)
                        .key("home-open-project"),
                ),
        );
    }
    content
}

pub(super) fn open_project(s: &mut Workspace, tx: &mut Tx) {
    let generation = live(s).app.home.generation();
    tx.open_folder(
        FileDialogOptions {
            title: "Choose a project for Home".into(),
            ..Default::default()
        },
        move |s: &mut Workspace, result, _| {
            let l = live_mut(s);
            if l.app.home.generation() != generation {
                return;
            }
            match result {
                Ok(Some(path)) => l.app.home_open_project(path.to_string_lossy().into_owned()),
                Ok(None) => {}
                Err(error) => l.app.home.error = Some(error.to_string()),
            }
        },
    );
}

pub(super) fn provider(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let l = live(s);
    Select::choices(
        "home-provider",
        Provider::ALL
            .into_iter()
            .map(|p| Choice::new(p.as_str(), ui::provider_name(p)))
            .collect(),
    )
    .label("Provider")
    .current(Some(l.app.home_provider().as_str()))
    .open(l.home.menu == Menu::Provider)
    .on_toggle(|s| toggle_menu(s, Menu::Provider))
    .on_close(close_menu)
    .on_choose(|s: &mut Workspace, value, _| {
        let l = live_mut(s);
        if let Some(provider) = Provider::ALL.into_iter().find(|p| p.as_str() == value) {
            l.app.home_set_provider(provider);
        }
        l.home.menu = Menu::Closed;
        l.focus();
    })
    .build(k, layers)
}

pub(super) fn model(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let options = l.app.home_options();
    let choices = l
        .app
        .home_scope()
        .and_then(|scope| l.app.choices.get(&scope));
    let current = options
        .model
        .as_ref()
        .or_else(|| choices.and_then(|c| c.default_model.as_ref()));
    let name = choices
        .and_then(|c| c.models.iter().find(|m| Some(&m.id) == current))
        .map(|m| m.name.as_str())
        .or(current.map(String::as_str));
    Select::new::<&str>("home-model", &[])
        .label("Model")
        .current(name)
        .placeholder("Provider default")
        .prominent()
        .disabled(!l.app.home_project_available())
        .open(l.models.is_open())
        .on_toggle(|s: &mut Workspace| crate::models::open(s, None))
        .trigger(k)
        .max_w(190.0)
}

pub(super) fn effort(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> Option<El> {
    let l = live(s);
    let options = l.app.home_options();
    let scope = l.app.home_scope()?;
    let choices = l.app.choices.get(&scope)?;
    let model = choices
        .models
        .iter()
        .find(|m| Some(&m.id) == options.model.as_ref().or(choices.default_model.as_ref()))?;
    if model.efforts.is_empty() {
        return None;
    }
    let mut efforts = vec![Choice::new("", "Default effort")];
    efforts.extend(model.efforts.iter().map(|e| Choice::new(&e.id, &e.name)));
    Some(
        Select::choices("home-effort", efforts)
            .label("Reasoning effort")
            .current(Some(options.effort.as_deref().unwrap_or("")))
            .open(l.home.menu == Menu::Effort)
            .on_toggle(|s| toggle_menu(s, Menu::Effort))
            .on_close(close_menu)
            .on_choose(|s: &mut Workspace, value, _| {
                let l = live_mut(s);
                let mut options = l.app.home_options();
                options.effort = (!value.is_empty()).then(|| value.into());
                l.app.home.draft.provider = Some(l.app.home_provider());
                l.app.home.draft.options = options;
                l.home.menu = Menu::Closed;
                l.focus();
            })
            .build(k, layers)
            .max_w(140.0),
    )
}

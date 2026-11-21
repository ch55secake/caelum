use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::remote::{Connection, Picker};
use rune::{El, FileDialogOptions, ScrollSpec, Tx, col};
use rune_kit::{ButtonVariant, Kit, ScrollBar};

#[derive(Default)]
pub(super) struct Folders {
    generation: u64,
    error: Option<String>,
}

impl Folders {
    // A dismissed native dialog must not select a folder for a later create flow.
    fn reset(&mut self) -> u64 {
        self.generation += 1;
        self.error = None;
        self.generation
    }
}

pub(crate) fn button(k: &Kit) -> El {
    ui::Button::new("Create VM…")
        .icon(&ui::SERVER)
        .variant(ButtonVariant::Contrast)
        .on_press(open)
        .build(k)
        .key("open-create-vm")
}

fn open(s: &mut Workspace) {
    let l = live_mut(s);
    l.app.dismiss_vm_handoff();
    l.remote.form = super::picker::Form::for_entry(l);
    l.panels.overlay = Some(Screen::CreateVm);
    l.remote.folders.reset();
    l.app.remote.picker = None;
    if l.app.remote.inventory.is_some() {
        if let Some(path) = l.app.active_project.clone() {
            l.app.remote_picker(path);
        }
    } else if matches!(l.app.remote.connection, Connection::Idle) {
        // A saved credential can be used even before Settings has been visited.
        l.app.list_machines();
    }
}

pub(crate) fn view(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    if l.app.remote.inventory.is_none() {
        return col()
            .w_full()
            .gap(10.0)
            .child(ui::section(
                "Connect Boat",
                "Connect your account to choose a project and create a VM.",
                k,
            ))
            .child(super::connection(s, k));
    }
    if l.app.remote.picker.is_none() {
        return projects(s, k);
    }
    super::picker(s, k)
}

pub(super) fn change_button(k: &Kit) -> El {
    ui::Button::new("Change project…")
        .variant(ButtonVariant::GhostMuted)
        .on_press(|s: &mut Workspace| {
            let l = live_mut(s);
            l.app.remote.picker = None;
            l.remote.form.reset_project();
            l.remote.folders.reset();
        })
        .build(k)
        .key("change-vm-project")
}

pub(crate) fn height(s: &Workspace) -> f32 {
    let l = live(s);
    if l.app.remote.inventory.is_none() {
        return 222.0;
    }
    match &l.app.remote.picker {
        None => 286.0,
        Some(Picker::Ready(source)) => {
            super::picker::existing_height(s, source).unwrap_or_else(|| {
                if rune::memo::window_layout("shell").is_some_and(|r| r[2] < 520.) {
                    420.0
                } else {
                    340.0
                }
            })
        }
        _ => 180.0,
    }
}

fn projects(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let mut items = col().w_full().gap(4.0);
    for project in l
        .app
        .projects
        .iter()
        .filter(|p| caelum_core::remote::native::checkout_id(&p.path).is_none())
    {
        let path = project.path.clone();
        items = items.child(
            ui::choice(&project.name, &path, false, k)
                .key(crate::code_view::source_key(&path, "vm-project"))
                .on_press(move |s: &mut Workspace| live_mut(s).app.remote_picker(path.clone())),
        );
    }
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(12.0)
        .child(ui::section(
            "Choose a project",
            "Choose a local checkout with a GitHub origin.",
            k,
        ))
        .children(
            l.app
                .projects
                .is_empty()
                .then(|| ui::caption("Open a project folder to get started.", k)),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(items)
                .scroll_bar(k),
        )
        .children(
            l.remote
                .folders
                .error
                .as_ref()
                .map(|e| ui::caption(e, k).wrap().color(k.c.danger.fg)),
        )
        .child(ui::divider(k))
        .child(
            ui::Button::new("Open project folder…")
                .icon(&ui::FOLDER)
                .on_press_tx(browse)
                .build(k)
                .key("browse-vm-project"),
        )
}

fn browse(s: &mut Workspace, tx: &mut Tx) {
    let generation = live_mut(s).remote.folders.reset();
    tx.open_folder(
        FileDialogOptions {
            title: "Choose a project for the VM".into(),
            ..Default::default()
        },
        move |s: &mut Workspace, result, _| {
            let l = live_mut(s);
            if l.remote.folders.generation != generation
                || l.panels.overlay != Some(Screen::CreateVm)
                || l.app.remote.picker.is_some()
            {
                return;
            }
            match result {
                Ok(Some(path)) => l.app.remote_picker(path.to_string_lossy().into_owned()),
                Ok(None) => {}
                Err(error) => l.remote.folders.error = Some(error.to_string()),
            }
        },
    );
}

mod changes;
mod checkout;
mod conversation;
mod create;
mod handoff;
mod launcher;
pub(crate) mod native;
mod picker;
mod popup;
mod provider;
mod scene;
mod setup;
mod sidebar;
mod stats;
mod status;
mod threads;
mod transcript;

pub(crate) use changes::{inspector, open as open_changes};
pub(crate) use checkout::enter_new;
pub(crate) use checkout::open_session_row;
pub(crate) use checkout::{screen, select, sync};
pub(crate) use conversation::focus;
pub(crate) use create::button as create_button;
pub(crate) use create::height as create_height;
pub(crate) use create::view as create_view;
pub(crate) use handoff::button as handoff_button;
pub(crate) use launcher::group as launcher_group;
pub(crate) use picker::{open, picker};
pub(crate) use popup::clear as close_popup;
pub(crate) use popup::view as popup;
pub(crate) use scene::scene;
pub(crate) use sidebar::{finish_removals, removal, request_removal, section as sidebar};
pub(crate) use threads::start as new_session;
pub(crate) use threads::view as threads_view;

use crate::{
    Workspace,
    live::{live, live_mut},
    settings::heading,
    ui,
};
use caelum_core::remote::Connection;
use rune::{El, TextEdit, col, row};
use rune_kit::Kit;

#[derive(Default)]
pub(crate) struct State {
    windows: native::Windows,
    key: TextEdit,
    form: picker::Form,
    folders: create::Folders,
    restored: bool,
    editors: std::collections::HashMap<String, conversation::Editors>,
    mounted: Option<(String, setup::Dock)>,
    menu: Option<provider::Menu>,
    inspection: Option<popup::Inspection>,
    pub(crate) removing: Option<(String, Option<caelum_core::remote::MachineId>)>,
    review: changes::Panel,
    checked_deadline: Option<(caelum_core::remote::MachineId, i64)>,
}

pub(crate) fn settings(s: &Workspace, k: &Kit) -> El {
    let mut view = col()
        .w_full()
        .gap(12.0)
        .child(
            row()
                .w_full()
                .center_y()
                .gap(12.0)
                .child(
                    heading("Remote machines", "Create and manage your Boat VMs.", k)
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(create_button(k)),
        )
        .child(connection(s, k));
    if let Some(list) = &live(s).app.remote.inventory {
        if list.machines.is_empty() {
            view = view.child(ui::caption("Connected · No machines yet", k));
        }
        for machine in &list.machines {
            view = view.child(ui::caption(
                format!("{} · {}", machine.name, machine.state),
                k,
            ));
        }
    }
    view
}

fn connection(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let remote = &l.app.remote;
    let busy = matches!(remote.connection, Connection::Loading(_));
    let mut view = col()
        .w_full()
        .gap(8.0)
        .child(
            ui::field(&l.remote.key, "Boat API key", "Boat API key", k)
                .secure()
                .key("boat-api-key")
                .on_edit(|s: &mut Workspace| &mut live_mut(s).remote.key),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    ui::Button::new("Connect Boat")
                        .disabled(busy || l.remote.key.text.trim().is_empty())
                        .on_press(|s: &mut Workspace| {
                            let l = live_mut(s);
                            let mut editor = std::mem::take(&mut l.remote.key);
                            l.app.connect_boat(std::mem::take(&mut editor.text));
                            l.schedule();
                        })
                        .build(k)
                        .key("connect-boat"),
                )
                .child(
                    ui::Button::new("Refresh")
                        .disabled(busy)
                        .on_press(|s: &mut Workspace| {
                            let l = live_mut(s);
                            l.app.list_machines();
                            l.schedule();
                        })
                        .build(k),
                )
                .children(remote.inventory.as_ref().map(|_| {
                    ui::Button::new("Disconnect")
                        .disabled(busy)
                        .on_press(|s: &mut Workspace| {
                            let l = live_mut(s);
                            l.app.disconnect_boat();
                            l.schedule();
                        })
                        .build(k)
                })),
        );
    if busy {
        view = view.child(ui::caption("Connecting to Boat…", k));
    }
    if let Connection::Failed(error) = &remote.connection {
        view = view.child(ui::caption(error, k).wrap().color(k.c.danger.fg));
    }
    view
}

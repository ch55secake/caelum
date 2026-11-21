use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use rune::{El, ScrollSpec, col, row};
use rune_kit::{ButtonVariant, Kit, ScrollBar};

pub(super) fn button(k: &Kit) -> El {
    ui::Button::new("Conversations…")
        .variant(ButtonVariant::GhostMuted)
        .on_press(|s: &mut Workspace| live_mut(s).panels.overlay = Some(Screen::VmThreads))
        .build(k)
        .key("vm-threads")
}

pub(crate) fn start(s: &mut Workspace, id: &str) {
    let l = live_mut(s);
    if !l.app.remote.saved.checkouts.contains_key(id) {
        return;
    }
    let provider = if l.panels.overlay == Some(Screen::Launcher) {
        l.panels.launch_provider
    } else {
        let project = caelum_core::remote::native::project_key(id);
        l.app
            .selected
            .map(|i| &l.app.sessions[i])
            .filter(|s| s.scope.project == project)
            .map_or_else(
                || l.app.default_provider_for(Some(&project)),
                |s| s.scope.provider,
            )
    };
    if l.panels.overlay == Some(Screen::Launcher)
        && l.app.remote.saved.selected.as_deref() != Some(id)
        && l.app
            .active_project
            .as_deref()
            .and_then(caelum_core::remote::native::checkout_id)
            != Some(id)
    {
        let source = &l.app.remote.saved.checkouts[id].source.project;
        let project = l
            .app
            .active_project
            .as_deref()
            .filter(|p| {
                caelum_core::remote::native::checkout_id(p).is_none()
                    && l.app.group_path(p) == l.app.group_path(source)
            })
            .unwrap_or(source)
            .to_owned();
        l.app.check_vm_destination(id, project);
    }
    super::checkout::session(s, id, Some(provider));
}

pub(crate) fn view(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let Some((id, checkout)) = app
        .remote
        .saved
        .selected
        .as_ref()
        .and_then(|id| app.remote.saved.checkouts.get_key_value(id))
    else {
        return col();
    };
    let mut body = col().w_full().h_full().min_h(0.0).gap(6.0).child(
        ui::caption(
            format!(
                "Current · {} · {}",
                checkout.thread.provider.name(),
                checkout.thread.title()
            ),
            k,
        )
        .truncate(),
    );
    let available = app.can_switch_vm_thread(id);
    if !available {
        body = body.child(
            ui::caption(
                "Finish the current VM operation before switching conversations.",
                k,
            )
            .wrap(),
        );
    }
    let owner = id.clone();
    let keys = owner.clone();
    let kit = k.clone();
    let rows = col()
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            checkout.other_threads.len(),
            48.0,
            move |s: &Workspace, i| {
                let thread = &live(s).app.remote.saved.checkouts[&keys].other_threads[i];
                format!("vm-thread-{}", thread.editor_key(&keys)).into()
            },
            move |s: &Workspace, i| {
                let thread = &live(s).app.remote.saved.checkouts[&owner].other_threads[i];
                let key = thread.editor_key(&owner).to_owned();
                let id = owner.clone();
                ui::choice(thread.title(), thread.provider.name(), false, &kit)
                    .disabled(!available)
                    .focusable(available)
                    .on_press(move |s: &mut Workspace| {
                        let l = live_mut(s);
                        if l.app.switch_vm_thread(&id, &key) {
                            l.panels.overlay = None;
                            super::conversation::sync_editor(l);
                            super::conversation::focus(l);
                        }
                    })
            },
        );
    let new = id.clone();
    body.child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows)
            .scroll_bar(k),
    )
    .child(ui::divider(k))
    .child(
        row().w_full().justify_end().child(
            ui::Button::new("New Boat conversation")
                .icon(&crate::PLUS)
                .variant(ButtonVariant::Contrast)
                .disabled(!app.can_start_vm_session(id))
                .on_press(move |s: &mut Workspace| {
                    let l = live_mut(s);
                    let provider = l.app.remote.saved.checkouts[&new].thread.provider;
                    if l.app.new_vm_session(&new, provider) {
                        super::checkout::enter_new(s);
                    }
                })
                .build(k)
                .key("new-vm-session")
                .help("Start a managed Boat conversation on this checkout."),
        ),
    )
}

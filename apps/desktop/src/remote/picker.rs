mod fields;

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::{
    application::{Provider, VmDestination},
    remote::{Lifetime, Picker, Size, Source, valid_branch},
};
use rune::{El, ScrollSpec, col, icon, row, text};
use rune_kit::{ButtonVariant, Kit, ScrollBar, glyphs};

#[derive(Default)]
pub(super) struct Form {
    launcher_provider: Option<Provider>,
    size: Size,
    lifetime: Lifetime,
    tab: Tab,
}

#[derive(Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Existing,
    New,
}

impl Form {
    pub(super) fn for_entry(l: &Live) -> Self {
        Self {
            launcher_provider: (l.panels.overlay == Some(Screen::Launcher))
                .then_some(l.panels.launch_provider),
            ..Self::default()
        }
    }

    pub(super) fn reset_project(&mut self) {
        *self = Self {
            launcher_provider: self.launcher_provider,
            ..Self::default()
        };
    }

    fn provider(&self) -> Provider {
        self.launcher_provider.unwrap_or(Provider::OpenCode)
    }

    fn ready(&self, source: &Source) -> bool {
        source.published_tip.is_some() && valid_branch(&source.base)
    }
}

pub(crate) fn open(s: &mut Workspace, project: String) {
    let l = live_mut(s);
    l.app.dismiss_vm_handoff();
    l.remote.form = Form::for_entry(l);
    l.app.remote_picker(project);
    l.panels.overlay = Some(Screen::RemotePicker);
    l.schedule();
}

pub(crate) fn picker(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let screen = l.panels.overlay;
    let standalone = screen == Some(Screen::CreateVm);
    let handoff = screen == Some(Screen::VmHandoff);
    let source = match &l.app.remote.picker {
        Some(Picker::Ready(source)) => source,
        Some(Picker::Failed { project, error }) => {
            let project = project.clone();
            return col()
                .w_full()
                .gap(12.0)
                .child(ui::notice("Couldn’t open this repository on Boat", error, k).role("alert"))
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .flex_wrap()
                        .gap(6.0)
                        .child(
                            ui::Button::new("Retry")
                                .icon(&ui::REFRESH)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s).app.remote_picker(project.clone())
                                })
                                .build(k),
                        )
                        .children(standalone.then(|| super::create::change_button(k)))
                        .child(
                            ui::Button::new("Connection settings")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(|s: &mut Workspace| {
                                    let l = live_mut(s);
                                    l.panels.settings_page = crate::settings::Page::Remote;
                                    l.panels.overlay = Some(Screen::Settings);
                                })
                                .build(k),
                        ),
                );
        }
        _ => {
            return col()
                .w_full()
                .gap(8.0)
                .child(
                    ui::busy_text("Looking up the repository on Boat…", k.ty.ui, k).role("status"),
                )
                .children(standalone.then(|| super::create::change_button(k)));
        }
    };

    let tabs = !standalone && has_machines(s, source);
    let existing = tabs && l.remote.form.tab == Tab::Existing;
    let mut pane = col().w_full().h_full().min_h(0.0).gap(8.0);
    if tabs {
        pane = pane.child(
            row()
                .role("tablist")
                .label("VM destination")
                .gap(4.0)
                .child(
                    ui::highlighted_tab("Existing VM", existing, k)
                        .key("handoff-existing-vm")
                        .on_press(|s: &mut Workspace| live_mut(s).remote.form.tab = Tab::Existing),
                )
                .child(
                    ui::highlighted_tab("New VM", !existing, k)
                        .key("handoff-new-vm")
                        .on_press(|s: &mut Workspace| live_mut(s).remote.form.tab = Tab::New),
                ),
        );
    }
    let mut settings = col()
        .w_full()
        .gap(12.0)
        .child(repository(source, standalone, existing, k));
    if existing {
        settings = settings.child(machines(s, k, source, handoff));
    } else {
        settings = settings
            .child(fields::checkout(k, source))
            .child(fields::machine(s, k));
    }
    pane = pane.child(
        col()
            .key("vm-settings-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(settings)
            .scroll_bar(k),
    );
    if existing {
        return pane;
    }

    let owner = source.clone();
    let provider = l
        .app
        .vm_handoff()
        .map_or(l.remote.form.provider(), |h| h.provider);
    pane.child(ui::divider(k)).child(
        row()
            .key("vm-create-footer")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(icon(&ui::AGENT, 14.0).color(k.c.icon_muted))
            .child(
                ui::caption(format!("{} · VM provider settings", provider.name()), k)
                    .truncate()
                    .min_w(0.0)
                    .grow(1.0)
                    .help("You can choose a different model in the VM composer before sending."),
            )
            .child(
                ui::Button::new("Create VM")
                    .variant(ButtonVariant::Contrast)
                    .disabled(!l.remote.form.ready(source))
                    .on_press(move |s: &mut Workspace| submit(s, &owner, screen))
                    .build(k)
                    .key("create-remote"),
            ),
    )
}

fn submit(s: &mut Workspace, owner: &Source, screen: Option<Screen>) {
    let l = live_mut(s);
    let handoff = screen == Some(Screen::VmHandoff);
    if l.panels.overlay != screen
        || !matches!(&l.app.remote.picker,Some(Picker::Ready(source)) if source==owner)
        || !l.remote.form.ready(owner)
    {
        return;
    }
    let f = &l.remote.form;
    if handoff {
        l.app.confirm_vm_handoff(VmDestination::Create {
            size: f.size,
            lifetime: f.lifetime,
        });
        l.schedule();
    } else if let Some(id) = l.app.create_remote(f.size, f.lifetime, f.provider()) {
        let provider = f.provider();
        super::checkout::session(s, &id, Some(provider));
    }
}

fn repository(source: &Source, standalone: bool, existing: bool, k: &Kit) -> El {
    let environment = if existing {
        "GitHub repository".into()
    } else {
        source.environment.as_ref().map_or_else(
            || "Creates a new Boat environment".into(),
            |name| format!("Boat environment · {name}"),
        )
    };
    row()
        .key("vm-repository")
        .w_full()
        .center_y()
        .gap(9.0)
        .child(
            row()
                .size(32.0, 32.0)
                .center()
                .radius(7.0)
                .fill(k.c.layer_02)
                .child(icon(&ui::FOLDER, 16.0).color(k.c.icon_muted)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(
                    text(&source.repository)
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .truncate()
                        .help(&source.project),
                )
                .child(ui::caption(&environment, k).truncate().help(if !existing && source.environment.is_none() {
                    "Creating this repository’s environment requires environment.write on the Boat key."
                } else { &environment })),
        )
        .children(standalone.then(|| super::create::change_button(k)))
}

fn has_machines(s: &Workspace, source: &Source) -> bool {
    live(s)
        .app
        .remote
        .inventory
        .as_ref()
        .is_some_and(|i| i.machines.iter().any(|m| m.key.account == source.account))
}

pub(super) fn existing_height(s: &Workspace, source: &Source) -> Option<f32> {
    let l = live(s);
    if l.panels.overlay != Some(Screen::RemotePicker) || l.remote.form.tab != Tab::Existing {
        return None;
    }
    let count = l
        .app
        .remote
        .inventory
        .as_ref()?
        .machines
        .iter()
        .filter(|m| m.key.account == source.account)
        .take(5)
        .count();
    (count > 0).then_some(144. + (count as f32) * 56.)
}

pub(crate) fn handoff_height(s: &Workspace) -> f32 {
    let l = live(s);
    if l.app.vm_handoff().is_none() || l.app.remote.inventory.is_none() {
        return 270.0;
    }
    if let Some(Picker::Ready(source)) = &l.app.remote.picker
        && has_machines(s, source)
        && l.remote.form.tab == Tab::Existing
    {
        return 296.0;
    }
    436.0
}

fn machines(s: &Workspace, k: &Kit, source: &Source, handoff: bool) -> El {
    let l = live(s);
    let app = &l.app;
    let new_session = l.remote.form.launcher_provider;
    let detail = format!(
        "The VM checkout must be on {} and contain its published commit.",
        source.base
    );
    let mut list = col().w_full().gap(6.0).child(ui::caption(detail, k).wrap());
    for machine in app
        .remote
        .inventory
        .iter()
        .flat_map(|i| &i.machines)
        .filter(|m| m.key.account == source.account)
    {
        let blocker = if handoff {
            app.handoff_machine_blocker(machine)
        } else if new_session.is_some() {
            app.vm_session_machine_blocker(machine)
        } else {
            None
        };
        let detail = blocker
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{} · {}", machine.state, machine.key.id));
        let owner = machine.clone();
        list = list.child(
            k.interactive(
                row()
                    .w_full()
                    .min_h(44.0)
                    .px(10.0)
                    .py(6.0)
                    .center_y()
                    .gap(9.0)
                    .radius(7.0)
                    .border(1.0, k.c.border_base)
                    .child(icon(&ui::SERVER, 15.0).color(k.c.icon_muted))
                    .child(
                        col()
                            .min_w(0.0)
                            .grow(1.0)
                            .gap(2.0)
                            .child(
                                text(&machine.name)
                                    .font(k.ty.ui_medium)
                                    .color(k.c.text_base)
                                    .truncate(),
                            )
                            .child(ui::caption(&detail, k).wrap()),
                    )
                    .child(icon(&glyphs::ARROW_RIGHT, 13.0).color(k.c.icon_muted)),
            )
            .role("button")
            .label(&machine.name)
            .description(&detail)
            .disabled(blocker.is_some())
            .focusable(blocker.is_none())
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .key(format!("attach-vm-{}", machine.key.id))
            .on_press(move |s: &mut Workspace| {
                let l = live_mut(s);
                if handoff {
                    l.app
                        .confirm_vm_handoff(VmDestination::Existing(owner.clone()));
                    l.schedule();
                } else if let Some(id) = l.app.attach_remote(owner.clone(), new_session) {
                    super::checkout::session(s, &id, new_session);
                }
            }),
        );
    }
    list
}

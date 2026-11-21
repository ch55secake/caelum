use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::application::{HandoffPhase, HandoffSource};
use rune::{El, ScrollSpec, col, row};
use rune_kit::{ButtonVariant, IconButtonVariant, Kit, ScrollBar};

pub(crate) fn button(s: &Workspace, k: &Kit, session: Option<usize>) -> El {
    let l = live(s);
    if l.native.is_some() {
        return rune::el();
    }
    let linked = session.and_then(|i| l.app.linked_vm(i));
    let blocker = session
        .filter(|_| linked.is_none())
        .and_then(|i| l.app.handoff_blocker(HandoffSource::Composer(i)));
    let label = linked.map_or_else(
        || "Continue on VM…".to_owned(),
        |link| {
            format!(
                "Machine stats · {}",
                l.app.remote.saved.checkouts[&link.checkout].title()
            )
        },
    );
    crate::quiet_button(&ui::SERVER, &label)
        .variant(if linked.is_some() {
            IconButtonVariant::Ghost
        } else {
            IconButtonVariant::GhostMuted
        })
        .expanded(matches!(
            l.panels.overlay,
            Some(Screen::VmHandoff | Screen::VmStats)
        ))
        .disabled(blocker.is_some())
        .on_press(move |s: &mut Workspace| {
            let l = live_mut(s);
            if let Some(i) = session
                && super::popup::inspect_local(l, i)
            {
                return;
            }
            l.app.dismiss_vm_handoff();
            l.models = Default::default();
            l.panels.overlay = Some(Screen::VmHandoff);
            if let Some(i) = session {
                begin(s, HandoffSource::Composer(i));
            }
        })
        .build(k)
        .size(32.0, 32.0)
        .key("continue-on-vm")
        .help(blocker.unwrap_or(&label))
}

pub(super) fn begin(s: &mut Workspace, source: HandoffSource) {
    let l = live_mut(s);
    if let Some(i) = source.session() {
        if let Some(editor) = l.drafts.get(i) {
            l.app.sessions[i].draft.clone_from(&editor.text);
        }
    } else {
        l.app.home.draft.text.clone_from(&l.home.editor.text);
    }
    l.remote.form = Default::default();
    l.panels.overlay = Some(Screen::VmHandoff);
    l.app.begin_vm_handoff(source);
    l.schedule();
}

pub(crate) fn view(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let Some(h) = l.app.vm_handoff() else {
        return if l.panels.screen == Screen::Home {
            sources(s, k)
        } else {
            ui::caption(
                "The local session changed. Reopen Continue on VM when it’s ready.",
                k,
            )
            .wrap()
        };
    };
    let mut body = col().w_full().h_full().min_h(0.0).gap(8.0).child(
        row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                rune::text(&h.title)
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base)
                    .truncate()
                    .min_w(0.0)
                    .grow(1.0),
            )
            .children((l.panels.screen == Screen::Home).then(|| {
                ui::Button::new("Change source…")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(|s: &mut Workspace| live_mut(s).app.dismiss_vm_handoff())
                    .build(k)
            })),
    );
    match &h.phase {
        HandoffPhase::Reading(_) => {
            return body.child(ui::busy_text("Reading local chat…", k.ty.ui, k).role("status"));
        }
        HandoffPhase::Checking(..) | HandoffPhase::Verified(..) => {
            return body
                .child(ui::busy_text("Checking the published branch…", k.ty.ui, k).role("status"));
        }
        HandoffPhase::Failed(error) => {
            let source = h.source();
            return body
                .child(ui::notice("Couldn’t prepare the handoff", error, k).role("alert"))
                .child(
                    ui::Button::new("Reload VM setup")
                        .on_press(move |s: &mut Workspace| begin(s, source))
                        .build(k),
                );
        }
        HandoffPhase::Ready(chat) => {
            let n = chat.messages.len();
            let summary = if chat.clipped {
                format!("First send includes the latest {n} messages.")
            } else if n == 0 {
                "Your draft will be ready before you send.".into()
            } else {
                format!(
                    "First send includes {n} chat {}.",
                    if n == 1 { "message" } else { "messages" }
                )
            };
            body = body.child(
                ui::caption(format!("{summary} Unpublished Git work stays local."), k).wrap(),
            );
        }
    }
    if l.app.remote.inventory.is_none() {
        body.child(ui::section(
            "Connect Boat",
            "Connect your account to choose a VM.",
            k,
        ))
        .child(super::connection(s, k))
    } else {
        body.child(super::picker(s, k).grow(1.0).min_h(0.0))
    }
}

fn sources(s: &Workspace, k: &Kit) -> El {
    let app = &live(s).app;
    let project = app.home.draft.project.as_deref();
    let blocker = app.handoff_blocker(HandoffSource::HomeDraft);
    let mut list = col().w_full().gap(4.0).child(
        ui::choice(
            "Home draft",
            blocker.or(project).unwrap_or_default(),
            false,
            k,
        )
        .key("vm-home-draft")
        .disabled(blocker.is_some())
        .focusable(blocker.is_none())
        .on_press(|s: &mut Workspace| begin(s, HandoffSource::HomeDraft)),
    );
    list = list.child(ui::caption("Recent sessions", k));
    let mut recent = app
        .session_feed(project, "", false)
        .into_iter()
        .filter(|&i| app.sessions[i].remote_id.is_some() || !app.sessions[i].draft.is_empty())
        .take(12)
        .peekable();
    if recent.peek().is_none() {
        list = list.child(ui::caption("No recent sessions in this project.", k));
    }
    for i in recent {
        let session = &app.sessions[i];
        let linked = app.linked_vm(i);
        let blocker = if linked.is_some() {
            None
        } else {
            app.handoff_blocker(HandoffSource::Recent(i))
        };
        let machine = linked.map(|v| {
            format!(
                "Machine stats · {}",
                app.remote.saved.checkouts[&v.checkout].title()
            )
        });
        list = list.child(
            ui::choice(
                &session.title,
                machine
                    .as_deref()
                    .or(blocker)
                    .unwrap_or(&session.scope.project),
                false,
                k,
            )
            .key(format!("vm-session-{i}"))
            .disabled(blocker.is_some())
            .focusable(blocker.is_none())
            .on_press(move |s: &mut Workspace| {
                if !super::popup::inspect_local(live_mut(s), i) {
                    begin(s, HandoffSource::Recent(i));
                }
            }),
        );
    }
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(list)
        .scroll_bar(k)
}

use super::setup::Dock;
use crate::{
    Workspace,
    context::EditMenu,
    live::{Live, live, live_mut},
    ui,
};
use caelum_core::remote::{Checkout, Job, Thread, Turn};
use rune::{El, Key, ScrollPos, TextEdit, TextStyle, Tx, col, icon, row, text, text_input};
use rune_kit::{ButtonVariant, Kit};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(super) struct Editors {
    draft: TextEdit,
    recovery: TextEdit,
    pub(super) reading: Rc<Cell<ScrollPos>>,
    draft_scroll: Rc<Cell<ScrollPos>>,
    pub(super) summary: Option<String>,
}

pub(crate) fn focus(l: &mut Live) {
    set_focus(l, true);
}

pub(super) fn set_focus(l: &mut Live, focused: bool) {
    if let Some((id, c)) = l
        .app
        .remote
        .saved
        .selected
        .as_ref()
        .and_then(|id| l.app.remote.saved.checkouts.get_key_value(id))
        && let Some(e) = l.remote.editors.get_mut(c.thread.editor_key(id))
    {
        e.draft.focused = focused;
    }
}

pub(super) fn restore(l: &mut Live, tx: &mut Tx) -> bool {
    let next = l.app.remote.saved.selected.as_ref().and_then(|id| {
        let c = l.app.remote.saved.checkouts.get(id)?;
        Some((c.thread.editor_key(id), Dock::for_checkout(c)))
    });
    let previous = l
        .remote
        .mounted
        .as_ref()
        .map(|(key, dock)| (key.as_str(), *dock));
    if next == previous {
        return false;
    }
    if let Some((key, dock)) = next
        && let Some(e) = l.remote.editors.get(key)
    {
        if previous.is_none_or(|(owner, _)| owner != key) {
            let p = e.reading.get();
            let region = format!("remote-transcript-{key}");
            if p.offset[1] >= p.max[1] - 1.0 {
                tx.scroll_to_bottom_instant(region);
            } else {
                tx.scroll_to_y(region, p.offset[1]);
            }
        }
        if dock == Dock::Draft {
            tx.scroll_to_y("composer-editor", e.draft_scroll.get().offset[1]);
        }
    }
    l.remote.mounted = next.map(|(key, dock)| (key.to_owned(), dock));
    true
}

pub(super) fn sync_editor(l: &mut Live) {
    let Some(id) = &l.app.remote.saved.selected else {
        return;
    };
    let Some(c) = l.app.remote.saved.checkouts.get(id) else {
        return;
    };
    let editors = l
        .remote
        .editors
        .entry(c.thread.editor_key(id).to_owned())
        .or_default();
    if editors.draft.text != c.thread.draft {
        editors.draft.replace_text(&c.thread.draft);
    }
}

fn changed(s: &mut Workspace, owner: &str) {
    let l = live_mut(s);
    if let Some(id) = &l.app.remote.saved.selected
        && let Some(c) = l.app.remote.saved.checkouts.get_mut(id)
        && c.thread.editor_key(id) == owner
    {
        c.thread
            .draft
            .clone_from(&l.remote.editors[owner].draft.text);
    }
}
fn send(s: &mut Workspace, tx: &mut Tx, owner: &str) {
    let l = live_mut(s);
    let Some(id) = l.app.remote.saved.selected.clone() else {
        return;
    };
    if l.app.remote.saved.checkouts[&id].thread.editor_key(&id) != owner {
        return;
    }
    let editor = &mut l.remote.editors.get_mut(owner).unwrap().draft;
    if l.app.send_remote(&id, &editor.text) {
        editor.clear();
        editor.focused = true;
        tx.scroll_to_bottom(format!("remote-transcript-{owner}"));
        l.schedule();
    }
}

pub(super) fn view(
    s: &Workspace,
    k: &Kit,
    id: &str,
    c: &Checkout,
    padding: f32,
    reduced: bool,
    layers: &mut Vec<El>,
) -> El {
    let l = live(s);
    let key = c.thread.editor_key(id);
    let branch = super::status::branch(c);
    let dock = Dock::for_checkout(c);
    let body = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .items_center()
        .child(ui::divider(k))
        .child(
            row()
                .w_full()
                .max_w(840.0)
                .px(padding)
                .h(42.0)
                .gap(7.0)
                .center_y()
                .child(
                    icon(
                        if branch.is_some() {
                            &ui::BRANCH
                        } else {
                            &ui::FOLDER
                        },
                        13.0,
                    )
                    .color(k.c.icon_muted),
                )
                .child(
                    ui::caption(branch.unwrap_or(&c.source.repository), k)
                        .truncate()
                        .grow(1.0)
                        .min_w(0.0)
                        .help(branch.map_or_else(
                            || c.source.repository.clone(),
                            |b| format!("{} · {b}", c.source.repository),
                        )),
                )
                .children((dock == Dock::Draft).then(|| super::native::button(s, k, id)))
                .children((dock == Dock::Draft).then(|| super::threads::button(k)))
                .children((dock == Dock::Draft).then(|| {
                    crate::quiet_button(&ui::FILE, "View remote changes")
                        .on_press(super::open_changes)
                        .build(k)
                        .key("remote-view-changes")
                })),
        )
        .child(super::transcript::view(s, k, id, c, padding, reduced));
    if dock == Dock::Setup {
        return body.child(super::setup::view(s, k, id, c, padding));
    }
    let Some(editors) = l.remote.editors.get(key) else {
        return body;
    };
    let busy = matches!(l.app.remote.feeds.get(id), Some(Job::Busy(_)));
    let status = super::status::status(&l.app.remote, id, c);
    let mut context = col().w_full().max_h(160.0).gap(6.0).child(
        row()
            .w_full()
            .center_y()
            .gap(6.0)
            .child(status.view(k).key("remote-turn-status").grow(1.0))
            .children(super::stats::pinned(s, k, id, c)),
    );
    let too_long = editors.draft.text.len() > Thread::MAX_MESSAGE_BYTES;
    if too_long {
        context = context.child(ui::caption(
            "Shorten your message to 64 KiB before sending.",
            k,
        ));
    }
    for error in [l.app.remote.jobs.get(id), l.app.remote.feeds.get(id)]
        .into_iter()
        .flatten()
    {
        if let Job::Failed(e) = error {
            context = context.child(ui::caption(e, k).wrap().color(k.c.danger.fg));
        }
    }
    if matches!(l.app.remote.jobs.get(id), Some(Job::Failed(_))) {
        let owner = id.to_owned();
        context = context.child(
            ui::Button::new("Refresh checkout")
                .icon(&ui::REFRESH)
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| live_mut(s).app.refresh_remote(&owner))
                .build(k)
                .key("refresh-remote-checkout"),
        );
    }
    if c.thread.turn.run().is_some() && matches!(l.app.remote.feeds.get(id), Some(Job::Failed(_))) {
        let owner = id.to_owned();
        context = context.child(
            ui::Button::new("Reconnect to turn")
                .icon(&ui::REFRESH)
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| live_mut(s).app.follow_remote(&owner))
                .build(k)
                .key("remote-reconnect"),
        );
    }
    if c.thread.turn == Turn::Submitting && !busy {
        let a = key.to_owned();
        let recovery = a.clone();
        let b = id.to_owned();
        let d = b.clone();
        context = context
            .child(ui::caption(
                "Check Boat for this message, then reconnect with its prompt ID.",
                k,
            ))
            .child(
                ui::field(&editors.recovery, "Boat prompt ID", "Prompt ID", k)
                    .key("remote-recovery")
                    .on_edit(move |s: &mut Workspace| {
                        &mut live_mut(s).remote.editors.get_mut(&a).unwrap().recovery
                    }),
            )
            .child(
                row()
                    .w_full()
                    .flex_wrap()
                    .gap(6.0)
                    .child(
                        ui::Button::new("Reconnect turn")
                            .on_press(move |s: &mut Workspace| {
                                let l = live_mut(s);
                                l.app
                                    .recover_remote(&b, &l.remote.editors[&recovery].recovery.text);
                            })
                            .build(k),
                    )
                    .child(
                        ui::Button::new("Dismiss submission")
                            .on_press(move |s: &mut Workspace| {
                                live_mut(s).app.dismiss_remote_submission(&d)
                            })
                            .build(k),
                    ),
            );
    }
    let owner = key.to_owned();
    let change_owner = owner.clone();
    let key_owner = owner.clone();
    let submit_owner = owner.clone();
    let provider = c.thread.provider.name();
    let input = text_input(&editors.draft, TextStyle::new(14.0, 400.0).lh(23.0))
        .label(format!("Message remote {provider}"))
        .placeholder(format!("Message {provider} on this VM…"), k.c.text_muted)
        .help("Enter to send · Shift+Enter for a new line")
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .edit_menu_changed(
            format!("remote-draft-{key}"),
            move |s| {
                live_mut(s)
                    .remote
                    .editors
                    .get_mut(&owner)
                    .map(|e| &mut e.draft)
            },
            move |s, _| changed(s, &change_owner),
        )
        .on_input_key(move |s: &mut Workspace, e, tx| {
            if e.key == Key::Enter && !e.mods.shift {
                send(s, tx, &key_owner);
                true
            } else {
                crate::on_key(s, e, tx)
            }
        })
        .on_submit(move |s, tx| send(s, tx, &submit_owner));
    let stopped = c
        .machine()
        .is_some_and(|m| matches!(m.state.as_str(), "archived" | "stopped"));
    let stats_owner = id.to_owned();
    let label = format!(
        "Machine stats · {}{}",
        c.title(),
        if stopped { " · Stopped" } else { "" }
    );
    let selectors = super::provider::controls(s, k, id, &c.thread, layers);
    let mut controls = row()
        .min_w(0.0)
        .gap(6.0)
        .center_y()
        .child(
            text(c.title())
                .font(k.ty.small_medium)
                .color(k.c.text_muted)
                .min_w(0.0)
                .grow(1.0)
                .truncate()
                .help(format!(
                    "Boat-managed {provider} on {} · {}.",
                    c.title(),
                    branch.unwrap_or(&c.source.repository)
                ))
                .key("remote-destination"),
        )
        .child(
            crate::quiet_button(&ui::SERVER, &label)
                .variant(rune_kit::IconButtonVariant::Ghost)
                .expanded(l.panels.overlay == Some(crate::panels::Screen::VmStats))
                .on_press(move |s: &mut Workspace| {
                    super::popup::inspect_remote(live_mut(s), &stats_owner)
                })
                .build(k)
                .size(32.0, 32.0)
                .key("remote-badge")
                .help(label),
        );
    if stopped {
        let owner = id.to_owned();
        controls = controls.child(
            ui::Button::new("Resume")
                .disabled(matches!(
                    l.app.remote.jobs.get(id),
                    Some(Job::Busy(_) | Job::Wait(_) | Job::Deleting(_))
                ))
                .on_press(move |s: &mut Workspace| live_mut(s).app.power_remote(&owner, true))
                .build(k),
        );
    } else if c.thread.turn.run().is_some() {
        let a = id.to_owned();
        let stopping =
            l.app.remote.interrupts.contains_key(id) || matches!(c.thread.turn, Turn::Stopping(_));
        controls = controls.child(
            ui::Button::new(if stopping { "Stopping…" } else { "Stop" })
                .icon(&crate::STOP)
                .variant(ButtonVariant::GhostMuted)
                .disabled(stopping || matches!(c.thread.turn, Turn::Finishing(_)))
                .on_press(move |s: &mut Workspace| live_mut(s).app.interrupt_remote(&a))
                .build(k)
                .key("remote-stop")
                .label("Stop this turn")
                .help("Stop the remote turn · Escape"),
        );
    } else {
        let owner = key.to_owned();
        controls = controls.child(
            ui::Button::new(if c.thread.turn == Turn::Submitting {
                "Sending…"
            } else {
                "Send"
            })
            .variant(ButtonVariant::Contrast)
            .icon(&crate::SEND)
            .disabled(
                c.thread.turn != Turn::Idle
                    || editors.draft.text.trim().is_empty()
                    || too_long
                    || l.app.remote.jobs.contains_key(id),
            )
            .on_press_tx(move |s, tx| send(s, tx, &owner))
            .build(k)
            .key("remote-send"),
        );
    }
    let draft_scroll = editors.draft_scroll.clone();
    let controls = if super::stats::wide() {
        row()
            .w_full()
            .center_y()
            .gap(6.)
            .child(selectors)
            .child(controls.grow(1.))
    } else {
        col()
            .w_full()
            .gap(4.)
            .child(selectors)
            .child(controls.w_full())
    };
    let mounted = l
        .remote
        .mounted
        .as_ref()
        .is_some_and(|(owner, dock)| owner == key && *dock == Dock::Draft);
    let height =
        rune::memo::window_layout("main-pane").map_or(220.0, |r| (r[3] * 0.35).clamp(96.0, 360.0));
    body.child(ui::composer(
        ui::composer_editor(input, 56.0, height).on_scroll_offset(move |p| {
            if mounted {
                draft_scroll.set(p);
            }
        }),
        controls,
        context,
        padding,
        k,
    ))
}

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::remote::VmLink;
use rune::{El, Tx};
use rune_kit::Kit;
use std::time::Instant;

fn dismiss_wait(l: &mut Live) {
    if let Some((id, _)) = l.remote.open_when_ready.take() {
        l.app.dismiss_vm_destination(&id);
    }
}

pub(crate) fn enter(s: &mut Workspace) {
    dismiss_wait(live_mut(s));
    if live(s).app.selected.is_some() && super::native::selected(live(s)).is_some() {
        let l = live_mut(s);
        l.panels.screen = Screen::Conversation;
        l.panels.overlay = None;
        l.sync_editors();
        l.focus();
        return;
    }
    if let Some(id) = live(s).app.remote.saved.selected.clone()
        && live(s).app.remote.saved.checkouts[&id]
            .thread
            .awaiting_native
    {
        session(s, &id, None);
        return;
    }
    show(s, true);
}

pub(crate) fn open_session_row(s: &mut Workspace, link: &VmLink) {
    dismiss_wait(live_mut(s));
    if live_mut(s).app.open_vm_thread(link) {
        if live(s).app.remote.saved.checkouts[&link.checkout]
            .thread
            .awaiting_native
        {
            session(s, &link.checkout, None);
            return;
        }
        // The Sessions list keeps keyboard focus so the next arrow selects the next row.
        show(s, false);
    }
}

fn show(s: &mut Workspace, focus_draft: bool) {
    let l = live_mut(s);
    l.panels.overlay = None;
    l.panels.screen = Screen::Remote;
    l.session_menu = None;
    l.remote.inspection = None;
    super::conversation::sync_editor(l);
    super::conversation::set_focus(l, focus_draft);
    l.schedule();
    s.inspector.set_open(false);
}

pub(crate) fn enter_new(s: &mut Workspace) {
    live_mut(s).reveal_new_session();
    enter(s);
}

pub(crate) fn select(s: &mut Workspace, id: &str) {
    live_mut(s).app.dismiss_vm_destination(id);
    session(s, id, None);
}

pub(super) fn session(
    s: &mut Workspace,
    id: &str,
    new: Option<caelum_core::application::Provider>,
) {
    if super::native::open(live_mut(s), id, new) {
        return;
    }
    if live(s).app.remote.saved.selected.as_deref() != Some(id) {
        live_mut(s).app.select_remote(id);
    }
    show(s, true);
    live_mut(s).remote.open_when_ready = Some((id.into(), new));
}

pub(crate) fn manage(s: &mut Workspace, id: &str) {
    live_mut(s).app.dismiss_vm_destination(id);
    dismiss_wait(live_mut(s));
    live_mut(s).app.select_remote(id);
    show(s, true);
}

pub(crate) fn sync(l: &mut Live, tx: &mut Tx) -> bool {
    let mut changed = false;
    if l.panels.overlay == Some(Screen::VmStats) && super::popup::owner(l).is_none() {
        l.panels.overlay = None;
        changed = true;
    }
    if l.panels.overlay != Some(Screen::VmStats) {
        l.remote.inspection = None;
    }
    if !l.remote.restored && l.app.restored() {
        l.remote.restored = true;
        if !l.app.opening
            && l.app.selected.is_none()
            && let Some(id) = l.app.remote.saved.selected.clone()
        {
            l.app.select_remote(&id);
            l.panels.screen = Screen::Remote;
            if l.app.remote.saved.checkouts[&id].thread.is_blank()
                || l.app.remote.saved.checkouts[&id].thread.awaiting_native
            {
                l.remote.open_when_ready = Some((id, None));
            }
            changed = true;
        }
    }
    if l.panels.screen != Screen::Remote && l.remote.restored {
        l.app.remote.saved.selected = None;
    }
    if let Some((id, new)) = l.remote.open_when_ready.clone() {
        if l.app.remote.saved.selected.as_deref() != Some(&id) || l.panels.screen != Screen::Remote
        {
            l.app.dismiss_vm_destination(&id);
            l.remote.open_when_ready = None;
        } else if l.panels.overlay.is_none()
            && !l.app.remote.jobs.contains_key(&id)
            && super::native::open(l, &id, new)
        {
            l.remote.open_when_ready = None;
            changed = true;
        }
    }
    changed |= super::provider::sync(l);
    changed |= l.app.dismiss_other_vm_tool();
    let now = Instant::now();
    super::stats::sync(l, tx, now);
    if let Some(at) = l.app.feed_deadline(now) {
        tx.wake_after(at.saturating_duration_since(now));
    }
    super::conversation::sync_editor(l);
    changed |= super::conversation::restore(l, tx);
    if let Some(at) = l.app.remote_deadline(now) {
        tx.wake_after(at.saturating_duration_since(now));
    }
    changed |= super::changes::sync(l);
    changed
}

pub(crate) fn screen(
    s: &Workspace,
    k: &Kit,
    padding: f32,
    reduced: bool,
    layers: &mut Vec<El>,
) -> El {
    let r = &live(s).app.remote;
    let Some((id, c)) = r
        .saved
        .selected
        .as_ref()
        .and_then(|id| r.saved.checkouts.get_key_value(id))
    else {
        return ui::empty("No remote checkout", "Choose a VM under its project.", k);
    };
    super::conversation::view(s, k, id, c, padding, reduced, layers)
}

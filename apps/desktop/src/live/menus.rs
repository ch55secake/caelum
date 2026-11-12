use rune::{El, col, icon, row, text};
use rune_kit::{ButtonVariant, Dialog, Kit, Placement, glyphs};

use crate::ui::Button;
use crate::{Workspace, quiet_button, ui};

use super::{Live, live, live_mut};

fn session_actions(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(index) = live.session_menu else {
        return col();
    };
    let session = &live.app.sessions[index];
    let mut content = col()
        .w_full()
        .gap(4.0)
        .child(ui::caption(&session.title, k).truncate().padding(8.0))
        .child(ui::divider(k))
        .child(
            ui::choice(
                "Refresh session",
                if session.loading {
                    "Reconnecting and loading history…"
                } else if session.remote_id.is_none() {
                    "Available after your first message"
                } else {
                    "Reconnect and load the latest history"
                },
                false,
                k,
            )
            .disabled(session.remote_id.is_none() || session.loading || session.submitting)
            .opacity(
                if session.remote_id.is_none() || session.loading || session.submitting {
                    0.5
                } else {
                    1.0
                },
            )
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.refresh(index);
                close_session_menu(s);
            })
            .role("menuitem")
            .focus_highlight(k.c.hover)
            .key("refresh-history"),
        );
    let archive_disabled = archive_disabled(live, index);
    content = content.child(
        ui::choice(
            if session.archived {
                "Restore session"
            } else {
                "Archive session"
            },
            if archive_disabled {
                "Finish active work, requests and queued messages first"
            } else {
                "Keep its history and return to it later"
            },
            false,
            k,
        )
        .disabled(archive_disabled)
        .role("menuitem")
        .focus_highlight(k.c.hover)
        .opacity(if archive_disabled { 0.5 } else { 1.0 })
        .on_press(move |s: &mut Workspace| {
            let archived = crate::live::live(s).app.sessions[index].archived;
            live_mut(s).archive(index, !archived);
        })
        .key("archive-session"),
    );
    if session.archived {
        content = content.child(
            ui::choice(
                if session.deleting {
                    "Deleting session…"
                } else {
                    "Delete permanently…"
                },
                "Delete this session and its provider history",
                false,
                k,
            )
            .disabled(archive_disabled || session.deleting)
            .role("menuitem")
            .focus_highlight(k.c.hover)
            .opacity(if archive_disabled || session.deleting {
                0.5
            } else {
                1.0
            })
            .on_press(move |s: &mut Workspace| {
                live_mut(s).request_delete_session(index);
            })
            .key("delete-session"),
        );
    }
    content
}

pub(crate) fn archive_disabled(live: &Live, index: usize) -> bool {
    let session = &live.app.sessions[index];
    session.working()
        || session.archiving
        || session.deleting
        || !session.queue.is_empty()
        || !session.inbox.items.is_empty()
        || live.app.approvals.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && p.session_id == session.remote_id
        })
        || live.app.questions.iter().any(|p| {
            p.project_path == session.scope.project
                && p.provider == session.scope.provider.as_str()
                && Some(&p.session_id) == session.remote_id.as_ref()
        })
}

pub fn close_session_menu(s: &mut Workspace) {
    let l = live_mut(s);
    if let Some(index) = l.session_menu.take() {
        l.focus_key = Some(format!("session-actions-{index}"));
    }
}

pub fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    super::sidebar::overlays(s, k, layers);
    let l = live(s);
    if let Some(index) = l.app.last_archived {
        let session = &l.app.sessions[index];
        let width =
            rune::memo::window_layout("main-pane").map_or(480.0, |r| (r[2] - 32.0).min(480.0));
        let toast = row()
            .key("archive-toast")
            .role("status")
            .label("Session archived")
            .w(width)
            .padding(10.0)
            .gap(10.0)
            .center_y()
            .on_hover_change(|s: &mut Workspace, inside, _| live_mut(s).archive_hovered = inside)
            .child(icon(&ui::ARCHIVE, 15.0).color(k.c.icon_muted))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(3.0)
                    .child(
                        text("Session archived")
                            .font(k.ty.ui_medium)
                            .color(k.c.text_base),
                    )
                    .child(ui::caption(&session.title, k).truncate())
                    .children(
                        session
                            .error
                            .as_ref()
                            .map(|e| ui::caption(e, k).color(k.c.danger.fg)),
                    ),
            )
            .child(
                Button::new(if session.archiving {
                    "Restoring…"
                } else {
                    "Undo"
                })
                .variant(ButtonVariant::GhostMuted)
                .disabled(session.archiving)
                .on_press(move |s: &mut Workspace| {
                    let l = live_mut(s);
                    if l.app.last_archived == Some(index) {
                        l.app.undo_archive();
                        l.sync_editors();
                    }
                })
                .build(k)
                .key("undo-archive"),
            )
            .child(
                quiet_button(&glyphs::CLOSE, "Dismiss archive notice")
                    .on_press(move |s: &mut Workspace| live_mut(s).app.dismiss_archive(index))
                    .build(k)
                    .key("dismiss-archive"),
            );
        let x =
            rune::memo::window_layout("main-pane").map_or(16.0, |r| r[0] + (r[2] - width) / 2.0);
        let bottom = rune::memo::window_layout("message-composer")
            .zip(rune::memo::window_layout("shell"))
            .map_or(18.0, |(composer, shell)| shell[3] - composer[1] + 12.0);
        layers.push(rune_kit::popover::raised(k, toast, 12.0).pin(
            Some(x),
            None,
            None,
            Some(bottom),
        ));
    }
    if let Some(index) = l.session_menu {
        ui::popup(
            "session-menu",
            &format!("session-actions-{index}"),
            session_actions(s, k).w(300.0),
            Placement::BottomStart,
            close_session_menu,
            k,
            layers,
        );
    }
}

pub fn delete_confirmation(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let Some(index) = live(s).delete_confirmation else {
        return;
    };
    let Some(session) = live(s).app.sessions.get(index) else {
        return;
    };
    let title = session.title.clone();
    let description = if session.remote_id.is_some() {
        format!(
            "Delete “{title}”, its provider history and any child sessions? This cannot be undone."
        )
    } else {
        format!(
            "Permanently remove “{title}” and its unsent draft and attachments? This cannot be undone."
        )
    };
    Dialog::new("delete-session", "Delete session permanently?")
        .open(true)
        .click_outside(false)
        .width(460.0)
        .fit()
        .on_close(|s: &mut Workspace| live_mut(s).cancel_delete_session())
        .child(
            col()
                .w_full()
                .gap(16.0)
                .px(16.0)
                .pb(16.0)
                .child(
                    text(description)
                        .key("delete-session-description")
                        .font(k.ty.ui.lh(20.0))
                        .color(k.c.text_muted)
                        .wrap()
                        .w_full(),
                )
                .child(
                    row()
                        .w_full()
                        .justify_end()
                        .gap(8.0)
                        .child(
                            crate::ui::Button::new("Cancel")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(|s: &mut Workspace| live_mut(s).cancel_delete_session())
                                .build(k)
                                .key("cancel-delete-session"),
                        )
                        .child(
                            crate::ui::Button::new("Delete permanently")
                                .variant(ButtonVariant::Contrast)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s).confirm_delete_session(index)
                                })
                                .build(k)
                                .key("confirm-delete-session"),
                        ),
                ),
        )
        .build(k, layers);
}

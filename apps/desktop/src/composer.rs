//! Session-owned editor position and previews of the exact context attached to a draft.
use std::{cell::Cell, rc::Rc};

use caelum_core::application::{Attachment, AttachmentKind};
use rune::{El, ScrollPos, Tx, col, row};
use rune_kit::{ButtonVariant, Kit, glyphs};

use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    quiet_button, ui,
};

#[derive(Default)]
pub struct Editor {
    pub position: Rc<Cell<ScrollPos>>,
}

pub struct Preview {
    pub owner: usize,
    pub attachment: Attachment,
    offset: f32,
}

#[derive(Default)]
pub struct State {
    pub editors: Vec<Editor>,
    pub preview: Option<Preview>,
    selected: Option<usize>,
}

pub fn editor(s: &Workspace, index: usize, input: El) -> El {
    let state = &live(s).composer.editors[index];
    let position = state.position.clone();
    // The first frame after switching sessions still has the previous viewport's
    // offset. Wait for poll's restoration before recording this session's scroll.
    let restored = live(s).composer.selected == Some(index);
    let pane = rune::memo::window_layout("main-pane").map_or(700.0, |r| r[3]);
    let controls = rune::memo::window_layout("composer-controls").map_or(102.0, |r| r[3]);
    let support = rune::memo::window_layout(&format!("composer-context-{index}"))
        .map_or(0.0, |r| r[3].min(180.0));
    // Grow with the draft, reserving a readable part of the conversation even
    // when attachments, queue controls or a narrow toolbar take more space.
    let height = (pane - controls - support - 65.0 - (pane * 0.3).max(140.0)).clamp(100.0, 420.0);
    ui::composer_editor(input, 56.0, height).on_scroll_offset(move |at| {
        if restored {
            position.set(at);
        }
    })
}

pub fn open(s: &mut Workspace, index: usize, id: &str) {
    let l = live_mut(s);
    let Some(attachment) = l.app.sessions[index]
        .attachments
        .iter()
        .find(|a| a.id == id)
        .cloned()
    else {
        return;
    };
    if attachment.kind == AttachmentKind::Image {
        l.media.selected = Some((
            l.app.sessions[index].scope.project.clone(),
            caelum_core::application::presentation::Media {
                name: attachment.label(),
                source: attachment.content.clone(),
            },
        ));
        l.media.zoom = None;
    }
    l.composer.preview = Some(Preview {
        owner: index,
        attachment,
        offset: l.composer.editors[index].position.get().offset[1],
    });
    l.panels.overlay = Some(Screen::Attachment);
}

pub fn sync(l: &mut crate::live::Live, tx: &mut Tx) -> bool {
    let mut changed = false;
    let selected = l
        .app
        .selected
        .filter(|_| l.panels.screen != Screen::Review && l.commands.side_owner.is_none());
    if l.composer.selected != selected {
        l.composer.selected = selected;
        if let Some(index) = selected {
            tx.scroll_to_y(
                "composer-editor",
                l.composer.editors[index].position.get().offset[1],
            );
        }
        changed = true;
    }
    if l.composer.preview.as_ref().is_some_and(|preview| {
        Some(preview.owner) != l.app.selected || l.panels.overlay != Some(Screen::Attachment)
    }) {
        let preview = l.composer.preview.take().unwrap();
        if l.panels.overlay == Some(Screen::Attachment) {
            l.panels.overlay = None;
        }
        if l.app.selected == Some(preview.owner) && l.panels.overlay.is_none() {
            l.focus();
            tx.scroll_to_y("composer-editor", preview.offset);
        }
        changed = true;
    }
    changed
}

pub fn attachments(s: &Workspace, k: &Kit, index: usize) -> El {
    let session = &live(s).app.sessions[index];
    let maximum =
        rune::memo::window_layout("message-composer").map_or(220.0, |r| (r[2] - 24.0).max(80.0));
    let mut chips = row().w_full().flex_wrap().gap(6.0);
    for attachment in &session.attachments {
        let id = attachment.id.clone();
        let remove = id.clone();
        let title = attachment.label();
        let icon_width = if attachment.kind == AttachmentKind::Image {
            22.0
        } else {
            14.0
        };
        let width = rune::text::measure(&title, &[], k.ty.small, None).0
            + icon_width
            + 6.0
            + 16.0
            + 24.0
            + 2.0
            + if attachment.truncated {
                rune::text::measure("Partial", &[], k.ty.small, None).0 + 6.0
            } else {
                0.0
            };
        let description = if attachment.truncated {
            "Partial snapshot · click to preview"
        } else {
            "Preview attached snapshot"
        };
        let preview = k
            .interactive(
                row()
                    .center_y()
                    .gap(6.0)
                    .px(8.0)
                    .h(32.0)
                    .grow(1.0)
                    .min_w(0.0)
                    .child(if attachment.kind == AttachmentKind::Image {
                        crate::media::attachment_thumbnail(s, k, attachment)
                    } else {
                        rune::icon(&ui::FILE, 14.0).color(k.c.icon_muted)
                    })
                    .child(ui::caption(&title, k).truncate().grow(1.0).min_w(0.0))
                    .children(
                        attachment
                            .truncated
                            .then(|| ui::caption("Partial", k).color(k.c.warning.fg)),
                    ),
            )
            .key(format!("composer-attachment-{id}"))
            .role("button")
            .label(format!("Preview {title}"))
            .help(format!("{title}\n{description}"))
            .focusable(true)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| open(s, index, &id));
        chips = chips.child(
            row()
                .center_y()
                .h(32.0)
                .radius(7.0)
                .w(width.ceil().min(maximum.min(320.0)))
                .fill(k.alpha(k.c.text_base, 0.035))
                .border(1.0, k.alpha(k.c.text_base, 0.06))
                .child(preview)
                .child(
                    quiet_button(&glyphs::CLOSE, &format!("Remove {title}"))
                        .on_press(move |s: &mut Workspace| {
                            live_mut(s).app.sessions[index]
                                .attachments
                                .retain(|a| a.id != remove);
                        })
                        .build(k)
                        .size(24.0, 28.0)
                        .key(format!("remove-attachment-{}", attachment.id)),
                ),
        );
    }
    let mut body = col().w_full().gap(6.0);
    if !session.attachments.is_empty() {
        body = body.child(chips);
    }
    for (request, name) in live(s).app.attachment_reads(index) {
        body = body.child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .key(format!("attachment-loading-{request}"))
                .child(ui::busy_text("Reading…", k.ty.small, k))
                .child(ui::caption(name, k).truncate().grow(1.0).min_w(0.0)),
        );
    }
    for (position, (name, error)) in session.attachment_errors.iter().enumerate() {
        let dismiss = (name.clone(), error.clone());
        body = body.child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .key(format!("attachment-error-{index}-{position}"))
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .gap(2.0)
                        .child(
                            ui::caption(format!("Could not attach {name}"), k)
                                .color(k.c.danger.fg)
                                .wrap(),
                        )
                        .child(ui::caption(error, k).wrap()),
                )
                .child(
                    quiet_button(&glyphs::CLOSE, "Dismiss attachment error")
                        .on_press(move |s: &mut Workspace| {
                            live_mut(s).app.sessions[index]
                                .attachment_errors
                                .retain(|entry| entry != &dismiss);
                        })
                        .build(k)
                        .key(format!("dismiss-attachment-error-{index}-{position}")),
                ),
        );
    }
    body
}

pub fn preview(s: &Workspace, k: &Kit) -> El {
    let Some(preview) = &live(s).composer.preview else {
        return col();
    };
    let attachment = &preview.attachment;
    if attachment.kind == AttachmentKind::Image {
        return crate::media::view(s, k);
    }
    let content = snapshot_content(attachment);
    let count = content.lines().count();
    let detail = attachment
        .start_line
        .zip(attachment.end_line)
        .map(|(start, end)| format!("Lines {start}–{end}"))
        .unwrap_or_else(|| format!("{count} line{}", if count == 1 { "" } else { "s" }));
    let copy = attachment.content.clone();
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(8.0)
        .child(crate::code_view::snapshot(
            &content,
            if attachment.kind == AttachmentKind::PullRequest {
                "pull-request.json"
            } else {
                &attachment.path
            },
            attachment.start_line.unwrap_or(1),
            k,
        ))
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    ui::caption(format!("Snapshot · {detail}"), k)
                        .truncate()
                        .grow(1.0)
                        .min_w(0.0),
                )
                .children(attachment.truncated.then(|| {
                    ui::caption("Partial", k).color(k.c.warning.fg).help(
                        "Partial snapshot: only the captured content shown here will be sent.",
                    )
                }))
                .child(
                    ui::Button::new("Copy")
                        .icon(&glyphs::COPY)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                        .build(k)
                        .key("copy-attachment-snapshot")
                        .label("Copy snapshot")
                        .help("Copy snapshot"),
                ),
        )
}

fn snapshot_content(attachment: &Attachment) -> String {
    if attachment.kind == AttachmentKind::PullRequest {
        serde_json::from_str::<serde_json::Value>(&attachment.content)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| attachment.content.clone())
    } else {
        attachment.content.clone()
    }
}

pub fn preview_header(s: &Workspace) -> (&str, &str) {
    let Some(preview) = &live(s).composer.preview else {
        return ("Attachment", "");
    };
    match preview.attachment.kind {
        AttachmentKind::Image => ("Image preview", ""),
        AttachmentKind::PullRequest => ("Pull request snapshot", &preview.attachment.path),
        AttachmentKind::File => {
            let (parent, name) = preview
                .attachment
                .path
                .rsplit_once('/')
                .unwrap_or(("", &preview.attachment.path));
            (name, parent)
        }
    }
}

pub fn preview_height(s: &Workspace) -> f32 {
    let Some(preview) = &live(s).composer.preview else {
        return 120.0;
    };
    if preview.attachment.kind == AttachmentKind::Image {
        return 480.0;
    }
    let lines = snapshot_content(&preview.attachment).lines().count();
    // Source lines use the same 20-point leading as Files; short snapshots fit
    // their content, while longer captures scroll below the filename header.
    (lines as f32 * 20.0 + 24.0).clamp(84.0, 420.0) + 56.0
}

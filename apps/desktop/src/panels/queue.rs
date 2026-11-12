use rune::{El, TextEdit, col, row};
use rune_kit::{ButtonVariant, Kit, glyphs};

use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, ui};

use super::label;

pub fn queue_summary(s: &Workspace, k: &Kit, index: usize) -> El {
    let l = live(s);
    let session = &l.app.sessions[index];
    let count = session.queue.len() + session.inbox.items.len();
    if count == 0 {
        return rune::el();
    }
    let failed = session.inbox.items.iter().any(|item| item.error.is_some())
        || (!session.queue.is_empty() && session.error.is_some());
    let status = if failed {
        " · Needs attention"
    } else if session.queue_paused && !session.queue.is_empty() {
        " · Paused"
    } else if session.inbox.items.iter().any(|item| item.pending) {
        " · Saving…"
    } else {
        ""
    };
    let title = format!("{count} queued{status}");
    let expanded = l.expanded_queues.contains(&index);
    k.interactive(
        row()
            .center_y()
            .gap(6.0)
            .h(24.0)
            .px(6.0)
            .radius(6.0)
            .child(
                ui::caption(&title, k)
                    .color(if failed {
                        k.c.danger.fg
                    } else {
                        k.c.text_muted
                    })
                    .truncate()
                    .min_w(0.0),
            )
            .child(ui::disclosure(expanded, k)),
    )
    .key("toggle-queued-messages")
    .role("button")
    .label(&title)
    .help(if expanded {
        "Hide queued messages"
    } else {
        "Show queued messages"
    })
    .expanded(expanded)
    .focusable(true)
    .focus_ring(1.5, 0.0, k.c.border_focus)
    .on_press(move |s: &mut Workspace| {
        let expanded = &mut live_mut(s).expanded_queues;
        if !expanded.remove(&index) {
            expanded.insert(index);
        }
    })
}

pub fn composer_context(s: &Workspace, k: &Kit, index: usize) -> El {
    let session = &live(s).app.sessions[index];
    let mut context = col().w_full().gap(4.0);
    if live(s).expanded_queues.contains(&index)
        && (!session.queue.is_empty() || !session.inbox.items.is_empty())
    {
        context = context.child(queued_messages(s, k, index));
    }
    if let Some(error) = &live(s).app.persistence_error {
        context = context.child(label(error, k));
    }
    context.child(crate::composer::attachments(s, k, index))
}

fn queued_preview(text: &str) -> &str {
    // Context payloads follow a blank line. Avoid decoding attachment snapshots
    // on every streamed frame just to show the queue's one-line summary.
    text.lines()
        .next()
        .filter(|line| !line.trim().is_empty())
        .unwrap_or("Attached context")
}

fn queued_messages(s: &Workspace, k: &Kit, index: usize) -> El {
    let session = &live(s).app.sessions[index];
    let mut context = col().key("queued-messages").w_full().gap(4.0);
    for item in &session.inbox.items {
        let id = item.id.clone();
        let retry = id.clone();
        let mut entry = row()
            .w_full()
            .gap(4.0)
            .center_y()
            .child(
                label(
                    format!(
                        "{}{}",
                        if item.pending {
                            "Saving… "
                        } else {
                            "Queued: "
                        },
                        queued_preview(&item.text)
                    ),
                    k,
                )
                .help(queued_preview(&item.text))
                .truncate()
                .grow(1.0)
                .min_w(0.0),
            )
            .child(
                Button::new("Remove")
                    .variant(ButtonVariant::GhostMuted)
                    .disabled(item.pending)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.remove_queued(index, &id))
                    .build(k)
                    .key(format!("remove-queued-{}", item.id)),
            );
        let mut item_view = col().w_full().gap(4.0);
        if let Some(error) = &item.error {
            entry = entry.child(
                Button::new("Retry")
                    .disabled(item.pending)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.retry_queued(index, &retry))
                    .build(k)
                    .key(format!("retry-queued-{}", item.id)),
            );
            item_view = item_view.child(label(error, k).color(k.c.danger.fg).wrap());
        }
        context = context.child(item_view.child(entry));
    }
    if !session.queue.is_empty()
        && let Some(error) = &session.error
    {
        context = context.child(label(error, k).color(k.c.danger.fg));
    }
    for (position, queued) in session.queue.iter().enumerate() {
        context = context.child(
            row()
                .w_full()
                .center_y()
                .gap(4.0)
                .child(
                    label(queued_preview(&queued.text), k)
                        .help(queued_preview(&queued.text))
                        .truncate()
                        .grow(1.0)
                        .min_w(0.0),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_UP, "Move queued message earlier")
                        .disabled(position == 0)
                        .on_press(move |s: &mut Workspace| {
                            let queue = &mut live_mut(s).app.sessions[index].queue;
                            if position > 0 && position < queue.len() {
                                queue.swap(position, position - 1);
                            }
                        })
                        .build(k)
                        .key(format!("move-queued-{index}-{position}")),
                )
                .child(
                    Button::new("Edit")
                        .variant(ButtonVariant::GhostMuted)
                        .disabled(
                            !live(s).drafts[index].text.is_empty()
                                || !session.attachments.is_empty()
                                || session.attaching > 0,
                        )
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            if position < live.app.sessions[index].queue.len()
                                && live.drafts[index].text.is_empty()
                                && live.app.sessions[index].attachments.is_empty()
                            {
                                let queued = live.app.sessions[index].queue.remove(position);
                                let (text, attachments) =
                                    caelum_core::application::split_context(&queued.text);
                                live.drafts[index] = TextEdit::new(text);
                                live.app.sessions[index].attachments = attachments;
                                live.app.sessions[index].options = queued.options;
                                live.app.options_changed(index);
                                live.drafts[index].focused = true;
                            }
                        })
                        .build(k)
                        .key(format!("edit-queued-{index}-{position}")),
                )
                .child(
                    Button::new("Remove")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(move |s: &mut Workspace| {
                            let queue = &mut live_mut(s).app.sessions[index].queue;
                            if position < queue.len() {
                                queue.remove(position);
                            }
                        })
                        .build(k)
                        .key(format!("remove-local-queued-{index}-{position}")),
                ),
        );
    }
    if session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Resume queued messages")
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut Workspace| live_mut(s).app.resume_queue(index))
                .build(k)
                .key("resume-queued-messages"),
        );
    }
    if !session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Pause queue")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.sessions[index].queue_paused = true
                })
                .build(k)
                .key("pause-queued-messages"),
        );
    }
    context
}

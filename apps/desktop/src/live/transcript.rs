use std::{collections::HashSet, rc::Rc};

use caelum_core::application::EntryKind;
use caelum_protocol::ApprovalDecision;
use rune::{El, ScrollSpec, TextEdit, col, icon, row, text};
use rune_kit::{ButtonVariant, Kit, Markdown, glyphs};

use crate::ui::Button;
use crate::{Workspace, quiet_button, theme, ui};

use super::{live, live_mut, open_project};

pub fn transcript(s: &Workspace, k: &Kit, reduced: bool, padding: f32, layers: &mut Vec<El>) -> El {
    if live(s).panels.screen == crate::panels::Screen::Review {
        return crate::panels::view(s, k, layers);
    }
    let live = live(s);
    let mut content = col().w_full().max_w(840.0).px(padding).py(24.0).gap(24.0);
    if let Some(error) = &live.app.persistence_error {
        content = content.child(text(error).font(k.ty.ui).color(k.c.text_accent).wrap());
    }
    if let Some(error) = &live.app.error {
        content = content.child(
            text(error.clone())
                .font(k.ty.ui)
                .color(k.c.text_accent)
                .wrap()
                .selectable(true),
        );
    }
    let Some(index) = live.app.selected else {
        return content
            .child(ui::empty(
                "A little room to think",
                "Open a project to work with Codex or OpenCode.",
                k,
            ))
            .child(
                Button::new("Open project")
                    .variant(ButtonVariant::Outline)
                    .on_press_tx(open_project)
                    .build(k)
                    .key("welcome-open"),
            )
            .grow(1.0);
    };
    let visible: Rc<Vec<_>> = Rc::new(
        live.app.sessions[index]
            .conversation
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| caelum_core::application::presentation::entry_visible(e))
            .map(|(i, _)| i)
            .collect(),
    );
    let count = visible.len();
    let keys = visible.clone();
    let rows = visible.clone();
    let reading = live.reading[index].clone();
    let kit = k.clone();
    col()
        .key(format!("live-transcript-{index}"))
        .role("region")
        .label("Conversation")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .items_center()
        .scroll(ScrollSpec { stick_bottom: true })
        .on_scroll_offset(move |position| reading.set(position))
        .child(
            col()
                .key(format!("live-rows-{index}"))
                .w_full()
                .max_w(840.0)
                .growth(rune::GrowthSpec::default())
                .virtual_rows(
                    count + 2,
                    130.0,
                    move |s: &Workspace, row| {
                        if row == 0 {
                            return format!("live-header-{index}").into();
                        }
                        match crate::live::live(s).app.sessions[index]
                            .conversation
                            .entries
                            .get(keys.get(row - 1).copied().unwrap_or(usize::MAX))
                        {
                            Some(entry) => format!("live-entry-{index}-{}", entry.id).into(),
                            None => format!("live-tail-{index}").into(),
                        }
                    },
                    move |s: &Workspace, row| {
                        timeline_row(
                            s,
                            &kit,
                            reduced,
                            padding,
                            index,
                            if row == 0 {
                                0
                            } else {
                                rows.get(row - 1).map_or(usize::MAX, |i| i + 1)
                            },
                        )
                    },
                )
                .on_visible_rows(move |s: &mut Workspace, range| {
                    let positions: Vec<_> = range
                        .filter_map(|row| row.checked_sub(1).and_then(|i| visible.get(i).copied()))
                        .collect();
                    let l = live_mut(s);
                    let entries = &l.app.sessions[index].conversation.entries;
                    let next: HashSet<_> = positions
                        .iter()
                        .filter_map(|i| entries.get(*i).map(|e| (index, e.id.clone())))
                        .collect();
                    let changed = next != l.visible_entries;
                    l.visible_entries = next;
                    if let Some(first) = positions.first().filter(|i| **i < entries.len()) {
                        l.current_prompt = entries[..=*first]
                            .iter()
                            .rev()
                            .find(|e| {
                                e.kind == EntryKind::User
                                    && caelum_core::application::presentation::entry_visible(e)
                            })
                            .map(|e| e.id.clone());
                    }
                    crate::media::request(s, index, &positions);
                    changed
                }),
        )
}

pub(super) fn timeline_row(
    s: &Workspace,
    k: &Kit,
    reduced: bool,
    padding: f32,
    index: usize,
    row_index: usize,
) -> El {
    let live = live(s);
    let mut content = col().w_full().px(padding).py(12.0).gap(16.0);
    let session = &live.app.sessions[index];
    if row_index == 0 {
        if let Some(error) = &live.app.error {
            content = content.child(text(error).font(k.ty.ui).color(k.c.text_accent).wrap());
        }
        if session.loading {
            content = content.child(ui::skeleton("Loading history", 3, k));
        }
        if session.next_cursor.is_some() {
            content = content.child(
                Button::new("Load older messages")
                    .variant(ButtonVariant::GhostMuted)
                    .disabled(session.loading)
                    .on_press(move |s: &mut Workspace| live_mut(s).app.older(index))
                    .build(k)
                    .key("load-older")
                    .self_start(),
            );
        }
        let needs_input = live.app.approvals.iter().any(|p| {
            p.provider == session.scope.provider.as_str()
                && p.project_path == session.scope.project
                && p.session_id == session.remote_id
        }) || live.app.questions.iter().any(|p| {
            p.provider == session.scope.provider.as_str()
                && p.project_path == session.scope.project
                && Some(&p.session_id) == session.remote_id.as_ref()
        });
        if session.conversation.entries.is_empty() && !session.loading && !needs_input {
            content = content.child(ui::empty(
                "A little room to think",
                "Ask a question, describe a change, or @mention a file.",
                k,
            ));
            if let Some(error) = live.app.notices.get(&session.scope) {
                content = content.child(
                    ui::notice(
                        &format!("Connect {}", ui::provider_name(session.scope.provider)),
                        error,
                        k,
                    )
                    .child(
                        Button::new("Provider settings")
                            .variant(ButtonVariant::GhostMuted)
                            .on_press(move |s: &mut Workspace| {
                                let l = live_mut(s);
                                l.panels.settings_page = crate::settings::Page::Providers;
                                l.panels.settings_provider = l.app.sessions[index].scope.provider;
                                crate::panels::show(s, crate::panels::Screen::Settings);
                            })
                            .build(k)
                            .self_start(),
                    ),
                );
            }
        }
        return content;
    }
    if let Some(entry) = session.conversation.entries.get(row_index - 1) {
        let mut item = col()
            .w_full()
            .gap(8.0)
            .key(format!("live-entry-{index}-{}", entry.id));
        match entry.kind {
            EntryKind::User => {
                let (_, files) = caelum_core::application::split_context(&entry.text);
                let message = caelum_core::application::presentation::entry_text(entry);
                let mut bubble =
                    ui::user_bubble(&message, !files.is_empty() || !entry.media.is_empty(), k)
                        .key(format!("user-bubble-{index}-{}", entry.id));
                if !message.trim().is_empty() {
                    bubble = bubble.child(
                        Markdown::new(&message)
                            .selectable(true)
                            .on_link_tx(move |s: &mut Workspace, url, tx| {
                                crate::panels::open_link(s, index, url, tx)
                            })
                            .on_link_secondary(move |s: &mut Workspace, url, at, tx| {
                                crate::context::link(s, index, url, at, tx)
                            })
                            .render(&ui::user_markdown(k)),
                    );
                }
                let mut attachments = col().w_full().gap(6.0);
                let has_files = !files.is_empty() || !entry.media.is_empty();
                for file in files {
                    if file.kind == caelum_core::application::AttachmentKind::Image {
                        attachments = attachments.child(crate::media::thumbnail(
                            s,
                            k,
                            &session.scope.project,
                            &caelum_core::application::presentation::Media {
                                name: file.path,
                                source: file.content,
                            },
                        ));
                        continue;
                    }
                    let path = file.path.clone();
                    attachments = attachments.child(
                        k.interactive(
                            row()
                                .w_full()
                                .max_w(600.0)
                                .gap(7.0)
                                .px(8.0)
                                .py(5.0)
                                .radius(6.0)
                                .fill(k.c.layer_01)
                                .child(icon(&ui::FILE, 13.0).color(k.c.icon_muted))
                                .child(
                                    text(format!(
                                        "{}{}",
                                        file.label(),
                                        if file.truncated { " · truncated" } else { "" }
                                    ))
                                    .font(k.ty.small)
                                    .color(k.c.text_muted)
                                    .truncate()
                                    .min_w(0.0),
                                ),
                        )
                        .role("button")
                        .label(file.label())
                        .focusable(true)
                        .focus_ring(1.0, 0.0, k.c.border_focus)
                        .on_press_tx(move |s: &mut Workspace, tx| {
                            crate::panels::open_link(s, index, &path, tx)
                        }),
                    );
                }
                for media in &entry.media {
                    attachments = attachments.child(crate::media::thumbnail(
                        s,
                        k,
                        &session.scope.project,
                        media,
                    ));
                }
                if has_files {
                    bubble = bubble.child(attachments);
                }
                let copy_key = format!("copy-user-{index}-{}", entry.id);
                let focused = live
                    .focus_key
                    .as_ref()
                    .is_some_and(|key| key.ends_with(&copy_key));
                let mut turn = row().w_full().justify_end().gap(6.0);
                if !message.is_empty() {
                    turn = turn.child(
                        quiet_button(&glyphs::COPY, "Copy message")
                            .on_press_tx(move |_: &mut Workspace, tx| tx.copy(message.clone()))
                            .build(k)
                            .key(copy_key)
                            .self_end()
                            .group_opacity(if focused { 1.0 } else { 0.0 }, 1.0),
                    );
                }
                item = item.hover_group().child(turn.child(bubble));
            }
            EntryKind::Assistant => {
                let id = entry.id.clone();
                item = item
                    .child(
                        row()
                            .w_full()
                            .center_y()
                            .justify_between()
                            .child(
                                row()
                                    .center_y()
                                    .gap(7.0)
                                    .child(icon(&glyphs::SPARKLE, 13.0).color(k.c.icon_muted))
                                    .child(
                                        text(if entry.complete {
                                            ui::provider_name(session.scope.provider)
                                        } else {
                                            "Responding…"
                                        })
                                        .font(k.ty.small)
                                        .color(k.c.text_muted),
                                    ),
                            )
                            .child(
                                quiet_button(&glyphs::COPY, "Copy response")
                                    .on_press_tx(move |s: &mut Workspace, tx| {
                                        if let Some(entry) = live_mut(s).app.sessions[index]
                                            .conversation
                                            .entries
                                            .iter()
                                            .find(|entry| entry.id == id)
                                        {
                                            tx.copy(
                                                caelum_core::application::presentation::entry_text(
                                                    entry,
                                                ),
                                            );
                                        }
                                    })
                                    .build(k)
                                    .key(format!("copy-response-{index}-{}", entry.id)),
                            ),
                    )
                    .child(
                        Markdown::new(caelum_core::application::presentation::entry_text(entry))
                            .selectable(true)
                            .streaming(!entry.complete && session.working())
                            .reveal(if reduced { 0.0 } else { 0.12 })
                            .on_link_tx(move |s: &mut Workspace, url, tx| {
                                crate::panels::open_link(s, index, url, tx)
                            })
                            .on_link_secondary(move |s: &mut Workspace, url, at, tx| {
                                crate::context::link(s, index, url, at, tx)
                            })
                            .render(&theme::markdown(k)),
                    );
                if entry.complete {
                    let agent = entry.metadata["agent"].as_str().map(|id| {
                        live.app
                            .choices
                            .get(&session.scope)
                            .and_then(|choices| choices.agents.iter().find(|agent| agent.id == id))
                            .map_or(id, |agent| agent.name.as_str())
                    });
                    let duration = entry.metadata["outputDurationMs"]
                        .as_u64()
                        .map(caelum_core::application::presentation::duration);
                    let details = match (agent, duration.as_deref()) {
                        (Some(agent), Some(duration)) => Some(format!("{agent} · {duration}")),
                        (Some(agent), None) => Some(agent.to_owned()),
                        (None, Some(duration)) => Some(duration.to_owned()),
                        (None, None) => None,
                    };
                    if let Some(details) = details {
                        item = item.child(
                            text(details)
                                .key(format!("output-details-{index}-{}", entry.id))
                                .font(k.ty.small)
                                .color(k.c.text_muted),
                        );
                    }
                }
            }
            EntryKind::Tool | EntryKind::Reasoning => {
                item = item.child(crate::activity::tool(s, k, index, entry));
            }
            EntryKind::Compaction => {
                item = item.child(crate::activity::compaction(s, k, index, entry));
            }
            EntryKind::Status => {
                item = item.child(
                    text(caelum_core::application::presentation::entry_text(entry))
                        .font(k.ty.small)
                        .color(k.c.text_muted)
                        .wrap(),
                );
            }
        }
        for media in entry.media.iter().filter(|_| entry.kind != EntryKind::User) {
            item = item.child(crate::media::thumbnail(s, k, &session.scope.project, media));
        }
        return content.child(crate::context::message(
            item,
            caelum_core::application::presentation::entry_text(entry),
            index,
            format!("live-entry-{index}-{}", entry.id),
        ));
    }
    if let Some(error) = &session.error {
        content = content.child(ui::notice("Session needs attention", error, k));
    }
    if let Some(prompt) = &session.failed_prompt {
        let prompt = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(8.0)
                .child(
                    text(format!("Unsent message\n{prompt}"))
                        .font(k.ty.ui)
                        .color(k.c.text_base)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    Button::new("Restore unsent message")
                        .variant(ButtonVariant::Outline)
                        .disabled(
                            !live.drafts[index].text.is_empty()
                                || !session.attachments.is_empty()
                                || session.attaching > 0,
                        )
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            if live.drafts[index].text.is_empty()
                                && live.app.sessions[index].attachments.is_empty()
                            {
                                let (text, attachments) =
                                    caelum_core::application::split_context(&prompt);
                                live.drafts[index] = TextEdit::new(text);
                                live.app.sessions[index].attachments = attachments;
                                live.drafts[index].focused = true;
                                live.app.sessions[index].failed_prompt = None;
                            }
                        })
                        .build(k)
                        .key("restore-message")
                        .self_start(),
                ),
        );
    }
    for prompt in live.app.approvals.iter().filter(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && (p.session_id.is_none() || p.session_id == session.remote_id)
    }) {
        let allow = prompt.clone();
        let deny = prompt.clone();
        content = content.child(
            col()
                .w_full()
                .gap(10.0)
                .padding(16.0)
                .radius(10.0)
                .border(1.0, k.c.border_focus)
                .child(
                    text(prompt.summary.clone())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .wrap(),
                )
                .child(
                    text(prompt.description.clone())
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .wrap()
                        .selectable(true),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(
                            Button::new("Allow once")
                                .variant(ButtonVariant::Outline)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(allow.clone(), ApprovalDecision::AllowOnce)
                                })
                                .build(k)
                                .key(format!("allow-once-{}", prompt.request_id)),
                        )
                        .child(
                            Button::new("Deny")
                                .variant(ButtonVariant::GhostMuted)
                                .on_press(move |s: &mut Workspace| {
                                    live_mut(s)
                                        .app
                                        .approve(deny.clone(), ApprovalDecision::Deny)
                                })
                                .build(k)
                                .key(format!("deny-{}", prompt.request_id)),
                        ),
                ),
        );
    }
    content
}

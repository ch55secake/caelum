use caelum_core::application::Task;
use caelum_protocol::GitHubFeedbackKind;
use rune::{El, Tx, col, row, text};
use rune_kit::{ButtonVariant, Kit, Markdown, diff, glyphs};

use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, theme, ui};

use super::action;

pub(super) fn jump_thread(s: &mut Workspace, id: &str, tx: &mut Tx) {
    let l = live_mut(s);
    let Some(thread) = l
        .panels
        .review
        .as_ref()
        .and_then(|key| l.app.services.reviews.get(key))
        .and_then(|r| r.threads.iter().find(|t| t.id == id))
    else {
        return;
    };
    l.panels.review_tab = 1;
    l.panels.review_restore = None;
    l.panels.review_file = Some(thread.path.clone());
    l.panels
        .expanded_threads
        .retain(|t| !t.starts_with("active:"));
    l.panels.expanded_threads.insert(format!("active:{id}"));
    l.panels.expanded_threads.insert(id.into());
    if let Some(i) = l.panels.diff.file_index(&thread.path) {
        if let Some(line) = thread.line.filter(|_| !thread.is_outdated) {
            l.panels.diff.scroll_to_line(
                i,
                diff::LineRef::new(
                    if thread.diff_side == "LEFT" {
                        diff::Side::Old
                    } else {
                        diff::Side::New
                    },
                    line as u32,
                ),
                tx,
            );
        } else {
            l.panels.diff.scroll_to_file(i, tx);
        }
    }
}

pub(super) fn comment_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    comment: &caelum_protocol::GitHubReviewComment,
) -> El {
    let key = (project.into(), url.into());
    let id = comment.id.clone();
    let selected = live(s)
        .app
        .services
        .reviews
        .get(&key)
        .is_some_and(|r| r.selected_comments.contains(&id));
    let link = comment.url.clone();
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(10.0)
        .fill(if selected { k.c.hover } else { k.c.layer_01 })
        .border(
            1.0,
            if selected {
                k.alpha(k.c.text_accent, 0.3)
            } else {
                k.c.border_base
            },
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    rune_kit::Checkbox::new(selected)
                        .label(format!(
                            "{} · {}",
                            comment
                                .author
                                .as_ref()
                                .map(|a| a.login.as_str())
                                .unwrap_or("Unknown"),
                            comment.state.as_deref().unwrap_or("Comment")
                        ))
                        .on_toggle(move |s: &mut Workspace| {
                            let review = live_mut(s).app.services.reviews.get_mut(&key).unwrap();
                            if !review.selected_comments.remove(&id) {
                                review.selected_comments.insert(id.clone());
                            }
                        })
                        .build(k)
                        .key(format!("select-comment-{}", comment.id)),
                )
                .child(rune::el().grow(1.0))
                .child(ui::caption(
                    comment.created_at.split('T').next().unwrap_or_default(),
                    k,
                ))
                .child(
                    quiet_button(&ui::EXTERNAL, "Open comment on GitHub")
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&link);
                        })
                        .build(k)
                        .label("Open comment on GitHub"),
                ),
        )
        .child(
            Markdown::new(&comment.body)
                .selectable(true)
                .render(&theme::markdown(k)),
        )
}

pub(super) fn thread_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    thread: &caelum_protocol::GitHubReviewThread,
) -> El {
    let open = !thread.is_resolved || live(s).panels.expanded_threads.contains(&thread.id);
    if !open {
        let id = thread.id.clone();
        return Button::new(format!(
            "Resolved · {}:{} · {} replies",
            thread.path,
            thread.line.or(thread.original_line).unwrap_or(0),
            thread.comments.len()
        ))
        .variant(ButtonVariant::GhostMuted)
        .icon(&glyphs::CARET_RIGHT)
        .on_press(move |s: &mut Workspace| {
            live_mut(s).panels.expanded_threads.insert(id.clone());
        })
        .build(k)
        .key(format!("thread-{}", thread.id));
    }
    let id = thread.id.clone();
    let mut content = col().w_full().gap(8.0).child(
        Button::new(format!(
            "{}:{} · {}{} · {}",
            thread.path,
            thread.line.or(thread.original_line).unwrap_or(0),
            if thread.is_resolved {
                "Resolved"
            } else {
                "Open"
            },
            if thread.is_outdated {
                " · Outdated"
            } else {
                ""
            },
            thread.diff_side
        ))
        .variant(ButtonVariant::GhostMuted)
        .on_press_tx(move |s: &mut Workspace, tx| jump_thread(s, &id, tx))
        .build(k)
        .key(format!("thread-location-{}", thread.id)),
    );
    if live(s).panels.review_tab == 0
        && let Some(hunk) = thread.comments.first().and_then(|c| c.diff_hunk.as_ref())
    {
        content = content.child(
            text(hunk)
                .font(k.ty.mono)
                .color(k.c.text_muted)
                .wrap()
                .selectable(true),
        );
    }
    for comment in &thread.comments {
        content = content.child(comment_view(s, k, project, url, comment));
    }
    if thread.is_resolved {
        let id = thread.id.clone();
        content = content.child(
            Button::new("Collapse resolved thread")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).panels.expanded_threads.remove(&id);
                })
                .build(k),
        );
    }
    if let Some(cursor) = &thread.next_cursor
        && let Some(head) = live(s)
            .app
            .services
            .reviews
            .get(&(project.into(), url.into()))
            .and_then(|r| r.head())
    {
        content = content.child(action(
            "More replies",
            Task::Feedback {
                project: project.into(),
                url: url.into(),
                head: head.into(),
                kind: GitHubFeedbackKind::Replies,
                cursor: Some(cursor.clone()),
                thread: Some(thread.id.clone()),
            },
            s,
            k,
        ));
    }
    content
}

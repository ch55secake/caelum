use super::live;
use crate::{Workspace, quiet_button, ui};
use rune::{El, col, icon, row, text};
use rune_kit::{Kit, glyphs};

pub(super) const HEIGHT: f32 = 88.0;

pub(super) fn view(s: &Workspace, k: &Kit, index: usize, position: usize, count: usize) -> El {
    let l = live(s);
    let app = &l.app;
    let session = &app.sessions[index];
    let selected = app.selected == Some(index);
    let project = crate::checkouts::project_name(app, app.group_path(&session.scope.project));
    let branch = app.session_branch(session).unwrap_or("No branch");
    let branch = app
        .session_worktree(session)
        .map_or_else(|| branch.to_owned(), |tree| format!("{tree} · {branch}"));
    let pending = app.approvals.iter().any(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && (p.session_id.is_none() || p.session_id == session.remote_id)
    }) || app.questions.iter().any(|p| {
        p.project_path == session.scope.project
            && p.provider == session.scope.provider.as_str()
            && Some(&p.session_id) == session.remote_id.as_ref()
    });
    let status = if pending {
        "Needs input"
    } else if session.archiving {
        "Archiving…"
    } else if session.deleting {
        "Deleting…"
    } else if session.stopping {
        "Stopping…"
    } else if session.working() {
        "Working…"
    } else if session.error.is_some() {
        "Error"
    } else if session.loading {
        "Loading…"
    } else if session.unread {
        "Unread"
    } else {
        ""
    };
    let provider = ui::provider_name(session.scope.provider);
    let age = super::age(session.last_activity, l.feed.now).0;
    let ink = if pending {
        k.c.warning.icon
    } else if session.error.is_some() {
        k.c.danger.fg
    } else {
        k.c.text_muted
    };
    let mark = if session.working() && !pending {
        ui::busy_icon(&ui::CLOCK, 10.0, k)
    } else {
        rune::el().size(5.0, 5.0).radius(3.0).fill(if pending {
            k.c.warning.icon
        } else if session.error.is_some() {
            k.c.danger.fg
        } else {
            k.c.text_accent
        })
    };
    let action_visible = selected
        || l.focus_key
            .as_ref()
            .is_some_and(|key| key.ends_with(&format!("session-actions-{index}")));
    let mut bottom = row()
        .w_full()
        .gap(4.0)
        .center_y()
        .child(icon(&ui::BRANCH, 11.0).color(k.c.icon_muted))
        .child(
            text(&branch)
                .font(k.ty.small)
                .color(k.c.text_muted)
                .truncate()
                .min_w(0.0)
                .grow(1.0),
        )
        .child(
            quiet_button(&ui::MORE, "Session actions")
                .on_press(move |s: &mut Workspace| super::live_mut(s).session_menu = Some(index))
                .build(k)
                .size(20.0, 20.0)
                .key(format!("session-actions-{index}"))
                .group_opacity(if action_visible { 1.0 } else { 0.0 }, 1.0),
        );
    if !super::super::archive_disabled(l, index) {
        bottom = bottom.child(
            quiet_button(&glyphs::CLOSE, "Close")
                .on_press(move |s: &mut Workspace| {
                    let l = super::live_mut(s);
                    if l.app.sessions[index].archived {
                        l.request_delete_session(index);
                    } else {
                        l.archive(index, true);
                    }
                })
                .build(k)
                .size(20.0, 20.0)
                .key("close")
                .focusable(false)
                .group_opacity(0.0, 1.0),
        );
    }
    let item = k
        .interactive(
            col()
                .w_full()
                .h(HEIGHT - 4.0)
                .px(10.0)
                .py(8.0)
                .gap(3.0)
                .hover_group()
                .child(
                    row()
                        .w_full()
                        .gap(5.0)
                        .center_y()
                        .child(l.feed.badges.view(app, &session.scope.project, k))
                        .child(
                            text(project)
                                .font(k.ty.small)
                                .color(k.c.text_muted)
                                .truncate()
                                .min_w(0.0)
                                .grow(1.0),
                        )
                        .children((!status.is_empty()).then_some(mark.a11y_hidden()))
                        .child(
                            text(provider)
                                .font(rune::TextStyle::new(10.0, 400.0))
                                .color(ink),
                        )
                        .child(
                            text(&age)
                                .key("session-age")
                                .font(k.ty.small)
                                .color(k.c.text_faint),
                        ),
                )
                .child(
                    text(&session.title)
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .truncate()
                        .w_full(),
                )
                .child(bottom),
        )
        .role("listitem")
        .label(&session.title)
        .description(format!("{project}, {branch}, {provider}, {status}, {age}"))
        .help(format!(
            "{}\n{branch}\n{provider} · {status}\n{}",
            session.scope.project,
            session.error.as_deref().unwrap_or("")
        ))
        .focusable(true)
        .focus_selected(selected)
        .focus_group()
        .focus_ring(1.0, 0.0, k.c.border_focus)
        .position_in_set(position + 1, count)
        .key(format!("live-session-{index}"))
        .on_press(move |s: &mut Workspace| super::select(s, index));
    crate::context::session(ui::selected_row(item, selected, k).mb(4.0), index)
}

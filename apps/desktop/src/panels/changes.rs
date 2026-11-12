use caelum_core::application::Task;
use rune::{El, col, row, text};
use rune_kit::{
    ButtonVariant, Kit,
    diff::{self, DiffMode, DiffState, DiffView},
};

use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, ui};

use super::feedback::thread_view;
use super::{label, scroll};

pub(super) fn diff_view(s: &Workspace, k: &Kit, review: bool) -> El {
    if !review
        && let Some(position) = live(s)
            .panels
            .change_file
            .as_ref()
            .and_then(|key| live(s).panels.change_positions.get(key))
    {
        position.capture("changes-diff");
    }
    if review
        && let Some(key) = &live(s).panels.review
        && let Some(position) = live(s).panels.review_positions.get(key)
    {
        position.capture("diff");
    }
    let kit = k.clone();
    let action_kit = k.clone();
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .child(
            Button::new(
                if (if review {
                    &live(s).panels.diff
                } else {
                    &live(s).panels.change_diff
                })
                .mode()
                    == DiffMode::Unified
                {
                    "Split diff"
                } else {
                    "Unified diff"
                },
            )
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let panels = &mut live_mut(s).panels;
                let diff = if review {
                    &mut panels.diff
                } else {
                    &mut panels.change_diff
                };
                diff.set_mode(if diff.mode() == DiffMode::Unified {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                });
            })
            .build(k)
            .self_start(),
        )
        .child(
            DiffView::new(
                move |s: &Workspace| {
                    if review {
                        &live(s).panels.diff
                    } else {
                        &live(s).panels.change_diff
                    }
                },
                move |s: &mut Workspace| {
                    if review {
                        &mut s.live.as_mut().unwrap().panels.diff
                    } else {
                        &mut s.live.as_mut().unwrap().panels.change_diff
                    }
                },
            )
            .key(if review { "diff" } else { "changes-diff" })
            .header_actions(move |s, i| {
                let l = live(s);
                let state = if review {
                    &l.panels.diff
                } else {
                    &l.panels.change_diff
                };
                let file = &state.files()[i];
                let path = file.path().to_owned();
                let Some(project) = (if review {
                    l.panels.review.as_ref().map(|v| v.0.clone())
                } else {
                    l.panels.change_file.as_ref().map(|v| v.0.clone())
                }) else {
                    return col();
                };
                if review
                    && let Some((key, file)) = l.panels.review.as_ref().and_then(|key| {
                        l.app
                            .services
                            .reviews
                            .get(key)?
                            .files
                            .iter()
                            .find(|f| f.path == path)
                            .map(|file| (key, file))
                    })
                    && file.patch.is_none()
                {
                    let url = format!("{}/files", key.1);
                    return Button::new("Patch unavailable")
                        .icon(&ui::EXTERNAL)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&url);
                        })
                        .build(&action_kit)
                        .help("Binary or omitted by GitHub. Open GitHub for the full file.")
                        .key(format!("unavailable-patch-{path}"));
                }
                let selected = state.selection().filter(|selection| selection.file == i);
                let content = selected
                    .and_then(|_| state.selected_text())
                    .unwrap_or_else(|| {
                        if review {
                            l.panels
                                .review
                                .as_ref()
                                .and_then(|key| l.app.services.reviews.get(key))
                                .and_then(|r| r.files.iter().find(|f| f.path == path))
                                .and_then(|f| f.patch.clone())
                                .unwrap_or_default()
                        } else {
                            l.app
                                .services
                                .checkouts
                                .get(&project)
                                .and_then(|c| c.diffs.get(&path))
                                .map(|d| format!("{}{}{}", d.staged, d.unstaged, d.untracked))
                                .unwrap_or_default()
                        }
                    });
                let copy = content.clone();
                if content.is_empty() {
                    return col();
                }
                crate::context::file(
                    Button::new(if selected.is_some() {
                        "Copy selection"
                    } else {
                        "Copy patch"
                    })
                    .variant(ButtonVariant::GhostMuted)
                    .on_press_tx(move |_: &mut Workspace, tx| tx.copy(copy.clone()))
                    .build(&action_kit)
                    .key(format!("patch-context-{path}")),
                    project.clone(),
                    path.clone(),
                    content,
                    selected.map(|s| {
                        (
                            s.start.line.min(s.end.line) as usize,
                            s.start.line.max(s.end.line) as usize,
                        )
                    }),
                    format!("patch-context-{path}"),
                )
            })
            .annotation(move |s, annotation| {
                let Some((project, url)) = &live(s).panels.review else {
                    return col();
                };
                let Some(thread) = live(s)
                    .app
                    .services
                    .reviews
                    .get(&(project.clone(), url.clone()))
                    .and_then(|r| r.threads.iter().find(|t| t.id == annotation.id))
                else {
                    return col();
                };
                thread_view(s, &kit, project, url, thread)
            })
            .on_visible_file(move |s, index| {
                if review {
                    let l = s.live.as_mut().unwrap();
                    // A partial refresh page must not replace the file we are
                    // restoring with its temporary first visible file.
                    if l.panels.review_restore.is_none() {
                        l.panels.review_file =
                            l.panels.diff.files().get(index).map(|f| f.path().into());
                    }
                    if index + 3 >= l.panels.diff.files().len()
                        && let Some((project, url)) = l.panels.review.clone()
                        && let Some(r) = l.app.services.reviews.get(&(project.clone(), url.clone()))
                        && let (Some(page), Some(head)) = (r.next_page, r.head())
                    {
                        l.app.task(Task::ReviewFiles {
                            project,
                            url,
                            head: head.into(),
                            page,
                        });
                    }
                }
            })
            .build(s, k)
            .focusable(true)
            .on_key(move |s: &mut Workspace, ev, tx| {
                let p = &mut live_mut(s).panels;
                crate::code_view::key(
                    if review {
                        &mut p.diff
                    } else {
                        &mut p.change_diff
                    },
                    ev,
                    tx,
                )
            }),
        )
}

pub(super) fn changes(s: &Workspace, k: &Kit) -> El {
    let Some(project) = &live(s).app.active_project else {
        return label("Open a project first", k);
    };
    let task = Task::Git {
        project: project.clone(),
    };
    let name = std::path::Path::new(project)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(project);
    let mut content = col().w_full().grow(1.0).min_h(0.0).gap(8.0).child(
        row()
            .w_full()
            .px(12.0)
            .pt(8.0)
            .center_y()
            .gap(7.0)
            .child(rune::icon(&ui::BRANCH, 16.0).color(k.c.icon_muted))
            .child(
                text(name)
                    .font(k.ty.ui)
                    .color(k.c.text_base)
                    .grow(1.0)
                    .truncate(),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh changes")
                    .disabled(live(s).app.services.busy(&task))
                    .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
                    .build(k)
                    .key("refresh-changes"),
            ),
    );
    if let Some(git) = live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.git.as_ref())
    {
        content = content.child(
            row()
                .w_full()
                .px(14.0)
                .center_y()
                .gap(6.0)
                .child(rune::icon(&ui::BRANCH, 12.0).color(k.c.icon_muted))
                .child(
                    ui::caption(
                        git.branch.as_deref().unwrap_or("Detached / non-repository"),
                        k,
                    )
                    .truncate()
                    .grow(1.0),
                )
                .child(ui::caption(
                    format!(
                        "{} {}",
                        git.changes.len(),
                        if git.changes.len() == 1 {
                            "file"
                        } else {
                            "files"
                        }
                    ),
                    k,
                )),
        );
        let mut files = col().w_full().px(8.0).gap(2.0);
        for change in &git.changes {
            let project = project.clone();
            let path = change.path.clone();
            let selected =
                live(s).panels.change_file.as_ref() == Some(&(project.clone(), path.clone()));
            files = files.child(
                k.interactive(
                    row()
                        .w_full()
                        .h(28.0)
                        .px(8.0)
                        .gap(8.0)
                        .center_y()
                        .radius(6.0)
                        .fill(if selected {
                            k.c.hover
                        } else {
                            rune::Color::TRANSPARENT
                        })
                        .child(
                            text(format!("{}{}", change.index_status, change.worktree_status))
                                .font(k.ty.small)
                                .color(k.c.warning.icon),
                        )
                        .child(
                            text(&path)
                                .font(rune::TextStyle {
                                    size: 11.0,
                                    ..k.ty.mono
                                })
                                .color(k.c.text_base)
                                .truncate()
                                .grow(1.0),
                        ),
                )
                .role("button")
                .label(&path)
                .focusable(true)
                .focus_ring(1.0, 0.0, k.c.border_focus)
                .on_press(move |s: &mut Workspace| {
                    select_change(s, project.clone(), path.clone());
                }),
            );
        }
        let height = (git.changes.len() as f32 * 28.0).clamp(28.0, 160.0);
        content = content.child(scroll(files).min_h(height).max_h(height));
        if git.changes.is_empty() {
            content = content.child(ui::empty(
                "Working tree clean",
                "Changes appear here as you and your agents edit.",
                k,
            ));
        }
    } else {
        return content.child(ui::skeleton("Loading changes", 4, k).padding(12.0));
    }
    if live(s)
        .panels
        .change_file
        .as_ref()
        .is_some_and(|(p, _)| p == project)
    {
        let ready = live(s).panels.change_file.as_ref().is_some_and(|(p, f)| {
            live(s)
                .app
                .services
                .checkouts
                .get(p)
                .is_some_and(|c| c.diffs.contains_key(f))
        });
        content = content.child(if ready {
            diff_view(s, k, false)
        } else {
            ui::skeleton("Loading patch", 5, k).padding(12.0)
        });
    } else if live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.git.as_ref())
        .is_some_and(|g| !g.changes.is_empty())
    {
        content = content.child(ui::caption("Select a file to review its patch", k).padding(16.0));
    }
    content
}

pub fn select_change(s: &mut Workspace, project: String, path: String) {
    let live = live_mut(s);
    if live.panels.change_file.as_ref() == Some(&(project.clone(), path.clone())) {
        return;
    }
    let position = live
        .panels
        .change_positions
        .entry((project.clone(), path.clone()))
        .or_default();
    live.panels.pending_scroll = Some((
        "changes-diff/scroll".into(),
        position.get().unwrap_or([0.0, 0.0]),
    ));
    live.panels.change_file = Some((project.clone(), path.clone()));
    live.panels.change_diff = DiffState::new(diff::parse(""));
    live.panels.change_hash = 0;
    live.app.task(Task::Diff { project, path });
}

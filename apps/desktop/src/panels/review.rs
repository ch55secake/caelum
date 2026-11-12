use caelum_core::application::Task;
use caelum_protocol::GitHubFeedbackKind;
use rune::{El, ScrollSpec, col, row, text};
use rune_kit::{ButtonVariant, Kit, Markdown, ScrollBar, diff, glyphs};

use crate::context::EditMenu;
use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, theme, ui};

use super::changes::diff_view;
use super::feedback::{comment_view, jump_thread, thread_view};
use super::{Screen, action, label, scroll};

pub fn open_review(s: &mut Workspace, project: String, url: String) {
    let live = live_mut(s);
    let key = (project.clone(), url.clone());
    if live.panels.review.as_ref() != Some(&key) {
        live.panels.review_restore = None;
        if let Some(previous) = live.panels.review.take() {
            live.panels.review_cache.insert(
                previous,
                (
                    std::mem::take(&mut live.panels.diff),
                    live.panels.diff_hash,
                    live.panels.annotations_hash,
                    live.panels.review_file.take(),
                ),
            );
        }
        let (diff, hash, notes, file) = live.panels.review_cache.remove(&key).unwrap_or_default();
        live.panels.diff = diff;
        live.panels.diff_hash = hash;
        live.panels.annotations_hash = notes;
        live.panels.review_file = file;
    }
    live.panels.screen = Screen::Review;
    live.panels.review = Some((project.clone(), url.clone()));
    live.app.task(Task::Details { project, url });
}

pub(super) fn review(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let Some((project, url)) = &live(s).panels.review else {
        return label("Choose a pull request", k);
    };
    let Some(review) = live(s)
        .app
        .services
        .reviews
        .get(&(project.clone(), url.clone()))
    else {
        return ui::skeleton("Loading pull request", 6, k);
    };
    let mut content = col().w_full().gap(12.0);
    if let Some(error) = &review.error {
        content = content.child(
            ui::notice("Review unavailable", error, k).child(
                action(
                    "Retry",
                    Task::Details {
                        project: project.clone(),
                        url: url.clone(),
                    },
                    s,
                    k,
                )
                .self_start(),
            ),
        );
    }
    if let Some(notice) = &review.notice {
        content = content.child(label(notice, k));
    }
    let Some(details) = &review.details else {
        return scroll(if review.error.is_some() {
            content
        } else {
            content.child(ui::skeleton("Loading overview", 6, k))
        });
    };
    let head = &details.head_ref_oid;
    let mut tabs = row().w_full().gap(6.0);
    for (tab, name) in [
        "Overview".to_owned(),
        format!("Files changed ({})", details.changed_files),
    ]
    .into_iter()
    .enumerate()
    {
        tabs = tabs.child(
            ui::tab(&name, live(s).panels.review_tab == tab, k)
                .on_press(move |s: &mut Workspace| live_mut(s).panels.review_tab = tab)
                .key(format!("review-tab-{tab}")),
        );
    }
    let link = url.clone();
    let refresh_project = project.clone();
    let refresh_url = url.clone();
    content = content
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    ui::caption(
                        if details.pull_request.is_draft {
                            "Draft"
                        } else {
                            &details.state
                        },
                        k,
                    )
                    .color(match details.state.as_str() {
                        "MERGED" => k.c.text_accent,
                        "CLOSED" => k.c.danger.fg,
                        _ if details.pull_request.is_draft => k.c.text_muted,
                        _ => k.c.success.fg,
                    })
                    .px(8.0)
                    .py(4.0)
                    .radius(6.0)
                    .fill(k.c.layer_01)
                    .border(1.0, k.c.border_base),
                )
                .child(ui::caption(
                    format!(
                        "#{} · opened by {}",
                        details.pull_request.number,
                        details
                            .author
                            .as_ref()
                            .map(|a| a.login.as_str())
                            .unwrap_or("Unknown")
                    ),
                    k,
                ))
                .child(rune::el().grow(1.0))
                .child(
                    Button::new("GitHub")
                        .icon(&ui::EXTERNAL)
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&link);
                        })
                        .build(k),
                )
                .child(
                    quiet_button(&ui::REFRESH, "Refresh review")
                        .on_press(move |s: &mut Workspace| {
                            let l = live_mut(s);
                            l.app
                                .services
                                .refresh_review(&refresh_project, &refresh_url);
                            l.app.task(Task::Details {
                                project: refresh_project.clone(),
                                url: refresh_url.clone(),
                            });
                        })
                        .build(k),
                ),
        )
        .child(
            text(&details.pull_request.title)
                .font(rune::TextStyle::new(22.0, 550.0))
                .color(k.c.text_base)
                .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(
                    text(&details.head_ref_name)
                        .font(k.ty.mono)
                        .color(k.c.text_accent)
                        .truncate()
                        .min_w(0.0)
                        .px(8.0)
                        .py(4.0)
                        .radius(5.0)
                        .fill(k.c.layer_02),
                )
                .child(rune::icon(&ui::BRANCH, 14.0).color(k.c.icon_muted))
                .child(
                    text(&details.pull_request.base_ref_name)
                        .font(k.ty.mono)
                        .color(k.c.text_muted)
                        .truncate()
                        .min_w(0.0)
                        .px(8.0)
                        .py(4.0)
                        .radius(5.0)
                        .fill(k.c.layer_02),
                )
                .child(rune::el().grow(1.0))
                .child(ui::caption(
                    format!("+{}  −{}", details.additions, details.deletions),
                    k,
                )),
        )
        .child(tabs)
        .child(ui::divider(k));
    let header = content;
    if live(s).panels.review_tab == 1 {
        return header
            .grow(1.0)
            .min_h(0.0)
            .child(review_files_view(s, k, project, url, review))
            .children(
                (!review.selected_comments.is_empty())
                    .then(|| review_actions(s, k, project, url, review)),
            );
    }
    let mut content = col().w_full().max_w(960.0).self_center().gap(24.0).pb(24.0);
    if live(s).panels.review_tab == 0 {
        content = content.child(
            col().w_full().py(8.0).child(
                Markdown::new(if details.body.trim().is_empty() {
                    "No description provided."
                } else {
                    &details.body
                })
                .selectable(true)
                .render(&theme::markdown(k)),
            ),
        );
    }
    content = content
        .child(crate::merge::view(s, k, project, url, details, layers))
        .child(
            text("Discussion & reviews")
                .font(k.ty.ui_medium)
                .color(k.c.text_base),
        );
    if review.discussion.is_empty() && review.reviews.is_empty() && review.threads.is_empty() {
        content = content.child(if review.cursors.len() < 3 && review.error.is_none() {
            ui::skeleton("Loading discussion", 3, k)
        } else {
            ui::caption(
                "No discussion yet. Review comments and replies will appear here.",
                k,
            )
            .py(12.0)
        });
    }
    let mut discussion = col().w_full().gap(10.0);
    for comment in review.discussion.iter().chain(&review.reviews) {
        discussion = discussion.child(comment_view(s, k, project, url, comment));
    }
    for thread in &review.threads {
        discussion = discussion.child(thread_view(s, k, project, url, thread));
    }
    content = content.child(discussion);
    for kind in [
        GitHubFeedbackKind::Discussion,
        GitHubFeedbackKind::Reviews,
        GitHubFeedbackKind::Threads,
    ] {
        if let Some(Some(cursor)) = review.cursors.get(&format!("{kind:?}")) {
            content = content.child(action(
                &format!("More {kind:?}"),
                Task::Feedback {
                    project: project.clone(),
                    url: url.clone(),
                    head: head.clone(),
                    kind,
                    cursor: Some(cursor.clone()),
                    thread: None,
                },
                s,
                k,
            ));
        }
    }
    header
        .max_w(960.0)
        .self_center()
        .grow(1.0)
        .min_h(0.0)
        .child(scroll(content).key("review-overview-scroll").scroll_bar(k))
        .children(
            (!review.selected_comments.is_empty())
                .then(|| review_actions(s, k, project, url, review)),
        )
}

fn review_actions(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    review: &caelum_core::application::Review,
) -> El {
    let index = live(s).app.selected;
    let destination = index
        .map(|i| live(s).app.sessions[i].title.as_str())
        .unwrap_or("No session selected");
    let blocker = index.map_or(Some("Choose a session in this checkout"), |i| {
        live(s).app.review_blocker(i, project)
    });
    let project = project.to_owned();
    let url = url.to_owned();
    let compact = rune::memo::window_layout("main-pane").is_some_and(|r| r[2] < 620.0);
    row()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .fill(k.c.layer_02)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(ui::caption(format!("Send to {destination}"), k).truncate())
                .child(
                    ui::caption(
                        blocker.unwrap_or("Selected feedback will start a new turn"),
                        k,
                    )
                    .truncate(),
                ),
        )
        .child(
            Button::new(if compact {
                format!(
                    "Address {} comment{}",
                    review.selected_comments.len(),
                    if review.selected_comments.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else {
                format!(
                    "Address {} selected comment{}",
                    review.selected_comments.len(),
                    if review.selected_comments.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            })
            .variant(ButtonVariant::Contrast)
            .disabled(blocker.is_some() || review.selected_comments.is_empty())
            .on_press_tx(move |s: &mut Workspace, tx| {
                let Some(index) = index else {
                    return;
                };
                let l = live_mut(s);
                if l.app.address_review(index, &project, &url) {
                    l.app.select(index);
                    l.panels.screen = Screen::Conversation;
                    l.focus();
                    tx.scroll_to_bottom_instant(format!("live-transcript-{index}"));
                }
            })
            .build(k)
            .key("address-comments"),
        )
}

fn review_files_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    review: &caelum_core::application::Review,
) -> El {
    let owner = (project.to_owned(), url.to_owned());
    let lookup = owner.clone();
    let kit = k.clone();
    let query = live(s).panels.review_search.text.to_lowercase();
    let indices: Vec<_> = review
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.path.to_lowercase().contains(&query))
        .map(|(i, _)| i)
        .collect();
    let keys = indices.clone();
    let files = col()
        .w_full()
        .growth(rune::GrowthSpec::default())
        .virtual_rows(
            indices.len(),
            50.0,
            move |s: &Workspace, i| {
                format!(
                    "review-file-{}",
                    live(s).app.services.reviews[&owner].files[keys[i]].path
                )
                .into()
            },
            move |s: &Workspace, i| {
                let r = &live(s).app.services.reviews[&lookup];
                let f = &r.files[indices[i]];
                let path = f.path.clone();
                let count = r.threads.iter().filter(|t| t.path == path).count();
                let detail = format!(
                    "{} · +{} −{}{}{}",
                    f.status,
                    f.additions,
                    f.deletions,
                    if count > 0 {
                        format!(" · {count} comments")
                    } else {
                        String::new()
                    },
                    if f.patch.is_none() {
                        " · Patch unavailable"
                    } else {
                        ""
                    }
                );
                let mut item = col().w_full().child(
                    ui::choice(
                        &path,
                        &detail,
                        live(s).panels.review_file.as_ref() == Some(&path),
                        &kit,
                    )
                    .min_h(44.0)
                    .on_press_tx(move |s: &mut Workspace, tx| {
                        let p = &mut live_mut(s).panels;
                        p.review_restore = None;
                        p.review_file = Some(path.clone());
                        if let Some(index) = p.diff.file_index(&path) {
                            p.diff.scroll_to_file(index, tx);
                        }
                    })
                    .h(48.0)
                    .w_full(),
                );
                for thread in r
                    .threads
                    .iter()
                    .filter(|t| t.path == f.path && !t.is_resolved)
                    .take(3)
                {
                    let id = thread.id.clone();
                    item = item.child(
                        Button::new(format!(
                            "Line {} · {}",
                            thread.line.or(thread.original_line).unwrap_or(0),
                            thread
                                .comments
                                .first()
                                .and_then(|c| c.author.as_ref())
                                .map(|a| a.login.as_str())
                                .unwrap_or("Thread")
                        ))
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |s: &mut Workspace, tx| jump_thread(s, &id, tx))
                        .build(&kit)
                        .key(format!("thread-link-{}", thread.id))
                        .pl(12.0),
                    );
                }
                item
            },
        );
    let compact = rune::memo::window_layout("main-pane").is_some_and(|r| r[2] < 640.0);
    let show_nav = live(s).panels.review_nav.unwrap_or(!compact);
    let nav = col()
        .key("review-file-list")
        .min_h(0.0)
        .h_full()
        .gap(8.0)
        .child(
            ui::field(
                &live(s).panels.review_search,
                "Find changed file",
                "Find a file…",
                k,
            )
            .edit_menu("review-file-search", |s| {
                Some(&mut live_mut(s).panels.review_search)
            }),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(files)
                .scroll_bar(k),
        );
    let nav = if compact {
        nav.w_full().h(126.0)
    } else {
        nav.w(224.0)
    };
    let mut code = if !review.files_loaded {
        ui::skeleton("Loading changed files", 6, k).grow(1.0)
    } else {
        diff_view(s, k, true)
    };
    if let Some(file) = review
        .files
        .iter()
        .find(|f| live(s).panels.review_file.as_ref() == Some(&f.path))
    {
        let mut fallback = col().w_full().gap(6.0);
        let mut has_fallback = false;
        if file.patch.is_none() {
            has_fallback = true;
            fallback = fallback.child(ui::caption(
                "Patch unavailable: binary or omitted by GitHub. Open GitHub for the full file.",
                k,
            ));
        }
        for thread in review.threads.iter().filter(|t| {
            t.path == file.path
                && (t.is_outdated
                    || t.line.is_none_or(|line| {
                        let side = if t.diff_side == "LEFT" {
                            diff::Side::Old
                        } else {
                            diff::Side::New
                        };
                        live(s).panels.diff.file_index(&file.path).is_none_or(|i| {
                            !crate::code_view::has_line(&live(s).panels.diff.files()[i], side, line)
                        })
                    }))
        }) {
            has_fallback = true;
            fallback = fallback.child(thread_view(s, k, project, url, thread));
        }
        if has_fallback {
            code = code.child(
                fallback
                    .max_h(220.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .key("review-file-notes"),
            );
        }
    }
    let mut toolbar = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            quiet_button(&rune_kit::shell::SIDEBAR, "Toggle changed files")
                .expanded(show_nav)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).panels.review_nav = Some(!show_nav);
                })
                .build(k)
                .key("review-nav-toggle"),
        )
        .child(
            ui::caption(
                live(s)
                    .panels
                    .review_file
                    .as_deref()
                    .unwrap_or("Changed files"),
                k,
            )
            .truncate()
            .grow(1.0)
            .min_w(0.0),
        );
    for (label, backwards) in [("Previous thread", true), ("Next thread", false)] {
        toolbar = toolbar.child(
            quiet_button(
                if backwards {
                    &ui::BACK
                } else {
                    &glyphs::CARET_RIGHT
                },
                label,
            )
            .disabled(review.threads.is_empty())
            .on_press_tx(move |s: &mut Workspace, tx| {
                let l = live(s);
                let Some(r) = l
                    .panels
                    .review
                    .as_ref()
                    .and_then(|key| l.app.services.reviews.get(key))
                else {
                    return;
                };
                let current = r.threads.iter().position(|t| {
                    l.panels
                        .expanded_threads
                        .contains(&format!("active:{}", t.id))
                });
                let next = if backwards {
                    current
                        .unwrap_or(0)
                        .checked_sub(1)
                        .unwrap_or(r.threads.len().saturating_sub(1))
                } else {
                    current.map_or(0, |i| (i + 1) % r.threads.len().max(1))
                };
                if let Some(id) = r.threads.get(next).map(|t| t.id.clone()) {
                    jump_thread(s, &id, tx);
                }
            })
            .build(k),
        );
    }
    let mut body = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(8.0)
        .child(toolbar)
        .child(
            (if compact { col() } else { row() })
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(10.0)
                .children(show_nav.then_some(nav))
                .child(code.grow(1.0).min_w(0.0)),
        );
    if let (Some(page), Some(head)) = (review.next_page, review.head()) {
        body = body.child(
            action(
                "Load more files",
                Task::ReviewFiles {
                    project: project.into(),
                    url: url.into(),
                    head: head.into(),
                    page,
                },
                s,
                k,
            )
            .key("review-more-files"),
        );
    }
    body
}

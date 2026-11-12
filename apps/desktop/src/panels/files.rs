use caelum_core::application::{Attachment, Task};
use caelum_protocol::{ProjectEntryKind, ProjectSearchMode};
use rune::{El, Key, ScrollSpec, Tx, col, row, text};
use rune_kit::{
    ButtonVariant, Kit, ScrollBar,
    diff::{self, DiffView},
    glyphs,
};

use crate::context::EditMenu;
use crate::live::{live, live_mut};
use crate::ui::Button;
use crate::{Workspace, quiet_button, ui};

use super::{Selection, action, label};

fn run_search(s: &mut Workspace, _: &mut Tx) {
    let live = live_mut(s);
    let Some(project) = live.app.active_project.clone() else {
        return;
    };
    live.app.task(Task::Search {
        project,
        query: live.panels.search.text.clone(),
        mode: if live.panels.content_search {
            ProjectSearchMode::Content
        } else {
            ProjectSearchMode::Files
        },
    });
}

pub(super) fn files(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(project) = &live.app.active_project else {
        return label("Open a project first", k);
    };
    let mut browser = col().w_full().gap(4.0).child(
        row()
            .gap(4.0)
            .center_y()
            .w_full()
            .child(
                ui::field(
                    &live.panels.search,
                    "Find files or text",
                    "Search files or text…",
                    k,
                )
                .grow(1.0)
                .min_w(0.0)
                .edit_menu("file-search", |s| Some(&mut live_mut(s).panels.search))
                .on_submit(run_search),
            )
            .child(
                quiet_button(&glyphs::SEARCH, "Find · Enter")
                    .on_press_tx(run_search)
                    .build(k)
                    .key("find-files"),
            )
            .child(
                quiet_button(&ui::REFRESH, "Refresh files")
                    .on_press(|s: &mut Workspace| live_mut(s).app.refresh_files())
                    .build(k),
            ),
    );
    browser = browser.child(
        row()
            .w_full()
            .gap(4.0)
            .child(
                ui::tab("Paths", !live.panels.content_search, k)
                    .h(26.0)
                    .on_press(|s: &mut Workspace| live_mut(s).panels.content_search = false),
            )
            .child(
                ui::tab("Content", live.panels.content_search, k)
                    .h(26.0)
                    .on_press(|s: &mut Workspace| live_mut(s).panels.content_search = true),
            )
            .child(rune::el().grow(1.0)),
    );
    let Some(checkout) = live.app.services.checkouts.get(project) else {
        return browser.child(ui::skeleton("Loading files", 4, k));
    };
    let mut listing = col().w_full().gap(2.0);
    if !live.panels.search.text.is_empty() {
        if let Some(results) = &checkout.search {
            for hit in &results.matches {
                let project = project.clone();
                let path = hit.path.clone();
                let line = hit.line;
                let context_project = project.clone();
                let context_path = path.clone();
                listing = listing.child(crate::context::path(
                    ui::choice(
                        &format!(
                            "{}{}",
                            hit.path,
                            hit.line.map(|l| format!(":{l}")).unwrap_or_default()
                        ),
                        hit.preview.as_deref().unwrap_or("Open file"),
                        false,
                        k,
                    )
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.panels.pending_line = line.map(|l| (project.clone(), path.clone(), l));
                        live.panels.selection = None;
                        live.app.open_file(project.clone(), path.clone());
                    }),
                    context_project,
                    context_path,
                    false,
                    format!("file-result-{}-{}", hit.path, line.unwrap_or_default()),
                ));
            }
            if results.truncated || results.skipped > 0 {
                listing = listing.child(label(
                    format!("Bounded results · {} files skipped", results.skipped),
                    k,
                ));
            }
        }
    } else {
        listing = listing.child(directory(s, k, project, "", 0));
    }
    browser = browser.child(
        col()
            .w_full()
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(listing)
            .h(140.0)
            .key("file-list")
            .scroll_bar(k),
    );
    let mut tabs = row()
        .key("file-tabs")
        .role("tablist")
        .label("Open files")
        .focus_group()
        .focus_follows()
        .w_full()
        .scroll_x();
    for path in &checkout.tabs {
        let project = project.clone();
        let path = path.clone();
        let close_path = path.clone();
        let close_project = project.clone();
        let selected = checkout.selected_file.as_ref() == Some(&path);
        let key = format!("file-tab-{}", crate::code_view::source_key(&project, &path));
        let close_key = format!("close-{key}");
        let context_project = project.clone();
        let context_path = path.clone();
        let context_key = key.clone();
        let focused = live
            .focus_key
            .as_ref()
            .is_some_and(|f| f == &key || f == &close_key);
        let name = path.rsplit('/').next().unwrap_or(&path);
        let width = (rune::text::measure(name, &[], k.ty.ui, None).0 + 62.0).clamp(100.0, 210.0);
        let close = quiet_button(&glyphs::CLOSE, "Close file")
            .on_press(move |s: &mut Workspace| {
                let c = live_mut(s)
                    .app
                    .services
                    .checkouts
                    .get_mut(&close_project)
                    .unwrap();
                c.tabs.retain(|p| p != &close_path);
                if c.selected_file.as_ref() == Some(&close_path) {
                    c.selected_file = c.tabs.last().cloned();
                }
            })
            .build(k)
            .key(close_key)
            .label(format!("Close {path}"))
            .size(22.0, 24.0)
            .group_opacity(if focused { 1.0 } else { 0.0 }, 1.0);
        tabs = tabs.child(crate::context::path(
            k.interactive(
                row()
                    .h(34.0)
                    .w(width)
                    .px(8.0)
                    .gap(6.0)
                    .center_y()
                    .fill(if selected {
                        k.c.layer_01
                    } else {
                        rune::Color::TRANSPARENT
                    })
                    .child(rune::icon(&ui::FILE, 13.0).color(if selected {
                        k.c.text_accent
                    } else {
                        k.c.icon_muted
                    }))
                    .child(
                        text(name)
                            .font(k.ty.ui)
                            .color(if selected {
                                k.c.text_base
                            } else {
                                k.c.text_muted
                            })
                            .truncate()
                            .min_w(0.0)
                            .grow(1.0),
                    )
                    .child(close)
                    .children(selected.then(|| {
                        rune::el().h(2.0).fill(k.c.text_accent).pin(
                            Some(0.0),
                            None,
                            Some(0.0),
                            Some(0.0),
                        )
                    })),
            )
            .key(key)
            .role("tab")
            .label(&path)
            .help(&path)
            .hover_group()
            .focus_group()
            .focus_return(if checkout.tabs.len() == 1 {
                "find-files"
            } else {
                "file-tabs"
            })
            .focusable(true)
            .focus_selected(selected)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let l = live_mut(s);
                l.panels.preview_page = 0;
                l.panels.selection = None;
                l.app.open_file(project.clone(), path.clone());
            }),
            context_project,
            context_path,
            false,
            context_key,
        ));
    }
    if !checkout.tabs.is_empty() {
        browser = browser.child(tabs).child(ui::divider(k));
    }
    let Some(path) = &checkout.selected_file else {
        if live.app.services.busy(&Task::Directory {
            project: project.clone(),
            path: String::new(),
            offset: 0,
        }) && !checkout.directories.contains_key("")
        {
            return browser.grow(1.0);
        }
        return browser
            .child(ui::empty(
                "Explore your project",
                "Choose a file to read, search or attach.",
                k,
            ))
            .grow(1.0);
    };
    let Some(file) = checkout.files.get(path) else {
        return browser
            .child(ui::skeleton("Loading preview", 4, k))
            .grow(1.0);
    };
    let project_attach = project.clone();
    let path_attach = path.clone();
    let has_selection = live
        .panels
        .selection
        .as_ref()
        .is_some_and(|selection| &selection.project == project && &selection.path == path);
    let mut actions = row().w_full().gap(4.0).center_y().child(
        Button::new(if has_selection {
            "Add selection"
        } else {
            "Add to prompt"
        })
        .variant(ButtonVariant::GhostMuted)
        .icon(&crate::PLUS)
        .disabled(live.app.selected.is_none_or(|i| {
            live.app.sessions[i].scope.project != *project
                || live.app.sessions[i].archived
                || live.app.sessions[i].forgotten
        }))
        .on_press(move |s: &mut Workspace| {
            let live = live_mut(s);
            let Some(index) = live
                .app
                .selected
                .filter(|i| live.app.sessions[*i].scope.project == project_attach)
            else {
                return;
            };
            let Some(file) = live
                .app
                .services
                .checkouts
                .get(&project_attach)
                .and_then(|c| c.files.get(&path_attach))
            else {
                return;
            };
            let selected = live.panels.selection.as_ref().filter(|selection| {
                selection.project == project_attach && selection.path == path_attach
            });
            let range = selected.and_then(|selection| selection.lines);
            let attachment = Attachment::new(
                file.path.clone(),
                selected
                    .map(|selection| selection.text.clone())
                    .unwrap_or_else(|| file.content.clone()),
                range,
                file.truncated,
            );
            live.app.attach(index, attachment);
        })
        .build(k)
        .h(28.0)
        .px(6.0)
        .key("attach-preview"),
    );
    if file.truncated {
        browser = browser.child(ui::caption("Partial file · preview size limit reached", k));
    }
    let refresh = Task::File {
        project: project.clone(),
        path: path.clone(),
    };
    actions = actions.child(rune::el().grow(1.0)).child(
        quiet_button(&ui::REFRESH, "Refresh preview")
            .disabled(live.app.services.busy(&refresh))
            .on_press(move |s: &mut Workspace| live_mut(s).app.task(refresh.clone()))
            .build(k)
            .key("refresh-file-preview"),
    );
    let owner = (project.clone(), path.clone());
    if !live.panels.documents.contains_key(&owner) {
        return browser.child(label("Preparing file…", k)).grow(1.0);
    }
    actions = actions.child(
        quiet_button(&glyphs::SEARCH, "Find in file · ⌘F")
            .on_press(|s: &mut Workspace| {
                let p = &mut live_mut(s).panels;
                p.finding = true;
                p.find.focused = true;
            })
            .build(k)
            .key("find-in-file"),
    );
    browser = browser.child(actions);
    if live.panels.finding {
        browser = browser.child(
            row()
                .w_full()
                .gap(4.0)
                .child(
                    ui::field(&live.panels.find, "Find in file", "Find in file…", k)
                        .label("Find in file")
                        .placeholder("Find in file…", k.c.text_muted)
                        .color(k.c.text_base)
                        .single_line()
                        .grow(1.0)
                        .min_w(0.0)
                        .edit_menu_changed(
                            "local-file-search",
                            |s| Some(&mut live_mut(s).panels.find),
                            |s, tx| {
                                live_mut(s).panels.find_index = usize::MAX;
                                find_next(s, false, tx);
                            },
                        )
                        .on_submit(|s: &mut Workspace, tx| find_next(s, false, tx)),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_UP, "Previous match")
                        .on_press_tx(|s: &mut Workspace, tx| find_next(s, true, tx))
                        .build(k),
                )
                .child(
                    quiet_button(&glyphs::CHEVRON_DOWN, "Next match")
                        .on_press_tx(|s: &mut Workspace, tx| find_next(s, false, tx))
                        .build(k),
                ),
        );
    }
    let get = owner.clone();
    let edit = owner.clone();
    let keyboard = owner.clone();
    let selected = owner.clone();
    let viewer = DiffView::new(
        move |s: &Workspace| &crate::live::live(s).panels.documents[&get].diff,
        move |s: &mut Workspace| {
            &mut s
                .live
                .as_mut()
                .unwrap()
                .panels
                .documents
                .get_mut(&edit)
                .unwrap()
                .diff
        },
    )
    .key(crate::code_view::source_key(project, path))
    .on_select(move |s: &mut Workspace, range, _| {
        let l = live_mut(s);
        l.panels.selection = range.map(|r| {
            let first = r.start.line.min(r.end.line) as usize;
            let last = r.start.line.max(r.end.line) as usize;
            let source = &l.app.services.checkouts[&selected.0].files[&selected.1].content;
            Selection {
                project: selected.0.clone(),
                path: selected.1.clone(),
                text: source
                    .split_inclusive('\n')
                    .skip(first.saturating_sub(1))
                    .take(last - first + 1)
                    .collect(),
                lines: Some((first, last)),
            }
        });
    })
    .build(s, k)
    .focusable(true)
    .on_key(move |s: &mut Workspace, ev, tx| {
        let p = &mut live_mut(s).panels;
        let handled =
            crate::code_view::key(&mut p.documents.get_mut(&keyboard).unwrap().diff, ev, tx);
        if handled && ev.key == Key::Escape {
            p.selection = None;
        }
        handled
    })
    .on_selection_change(move |s: &mut Workspace, change, _| {
        if let Some(selection) = change.selection() {
            live_mut(s).panels.selection = Some(Selection {
                project: owner.0.clone(),
                path: owner.1.clone(),
                text: selection.text().into(),
                lines: None,
            });
        } else {
            live_mut(s).panels.selection = None;
        }
    });
    let selection = live
        .panels
        .selection
        .as_ref()
        .filter(|s| &s.project == project && &s.path == path);
    let viewer = crate::context::file(
        viewer,
        project.clone(),
        path.clone(),
        selection
            .map(|s| s.text.clone())
            .unwrap_or_else(|| file.content.clone()),
        selection.and_then(|s| s.lines),
        crate::code_view::source_key(project, path),
    );
    live.panels.documents[&(project.clone(), path.clone())]
        .position
        .capture(&crate::code_view::source_key(project, path));
    browser.child(viewer).grow(1.0).min_h(0.0)
}

pub(super) fn find_next(s: &mut Workspace, backwards: bool, tx: &mut Tx) {
    let l = live_mut(s);
    let Some(project) = l.app.active_project.clone() else {
        return;
    };
    let Some(checkout) = l.app.services.checkouts.get(&project) else {
        return;
    };
    let Some(path) = checkout.selected_file.clone() else {
        return;
    };
    let Some(file) = checkout.files.get(&path) else {
        return;
    };
    let query = l.panels.find.text.to_lowercase();
    if query.is_empty() {
        return;
    }
    let lines: Vec<_> = file
        .content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(&query))
        .map(|(i, _)| i + 1)
        .collect();
    if lines.is_empty() {
        return;
    }
    let next = if l.panels.find_index == usize::MAX {
        0
    } else if backwards {
        (l.panels.find_index + lines.len() - 1) % lines.len()
    } else {
        (l.panels.find_index + 1) % lines.len()
    };
    l.panels.find_index = next;
    if let Some(document) = l.panels.documents.get_mut(&(project, path)) {
        let line = diff::LineRef::new(diff::Side::New, lines[next] as u32);
        document
            .diff
            .set_selection(Some(diff::LineRange::line(0, line)));
        document.diff.scroll_to_line(0, line, tx);
    }
}

fn directory(s: &Workspace, k: &Kit, project: &str, path: &str, depth: usize) -> El {
    let mut tree = col().w_full().gap(2.0);
    let Some(listing) = live(s)
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.directories.get(path))
    else {
        return if live(s).app.services.busy(&Task::Directory {
            project: project.into(),
            path: path.into(),
            offset: 0,
        }) {
            tree.child(ui::skeleton("Loading files", 2, k))
        } else {
            tree
        };
    };
    for entry in &listing.entries {
        let dir = entry.kind == ProjectEntryKind::Directory;
        let p = project.to_owned();
        let f = entry.path.clone();
        let expanded = live(s)
            .app
            .preferences
            .files
            .get(&p)
            .is_some_and(|saved| saved.expanded.contains(&f));
        tree = tree.child(crate::context::path(
            k.interactive(
                row()
                    .w_full()
                    .h(28.0)
                    .pl(6.0 + depth as f32 * 12.0)
                    .pr(6.0)
                    .gap(7.0)
                    .radius(6.0)
                    .center_y()
                    .child(
                        rune::icon(
                            if dir {
                                if expanded {
                                    &glyphs::CHEVRON_DOWN
                                } else {
                                    &glyphs::CARET_RIGHT
                                }
                            } else {
                                &ui::FILE
                            },
                            13.0,
                        )
                        .color(k.c.icon_muted),
                    )
                    .child(
                        text(&entry.name)
                            .font(k.ty.ui)
                            .color(k.c.text_base)
                            .truncate()
                            .grow(1.0),
                    ),
            )
            .role("button")
            .label(&entry.path)
            .focusable(true)
            .focus_ring(1.5, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                if dir {
                    let saved = live.app.preferences.files.entry(p.clone()).or_default();
                    if !saved.expanded.remove(&f) {
                        saved.expanded.insert(f.clone());
                        live.app.task(Task::Directory {
                            project: p.clone(),
                            path: f.clone(),
                            offset: 0,
                        });
                    }
                } else {
                    live.panels.preview_page = 0;
                    live.panels.selection = None;
                    live.app.open_file(p.clone(), f.clone());
                }
            })
            .key(format!("file-tree-{}", entry.path)),
            project.to_owned(),
            entry.path.clone(),
            dir,
            format!("file-tree-{}", entry.path),
        ));
        if dir && expanded && depth < 30 {
            tree = tree.child(directory(s, k, project, &entry.path, depth + 1));
        }
    }
    if let Some(offset) = listing.next_offset {
        tree = tree.child(action(
            "More files",
            Task::Directory {
                project: project.into(),
                path: path.into(),
                offset,
            },
            s,
            k,
        ));
    }
    tree
}

//! Native workspace views; all reads, mutations and response ownership are in core.
use crate::{
    Workspace,
    live::{live, live_mut},
    quiet_button, theme, ui,
};
use caelum_core::application::{Attachment, Task, Workspace as Application};
use caelum_protocol::{GitHubFeedbackKind, GitHubMergeMethod, ProjectEntryKind, ProjectSearchMode};
use rune::{
    El, FileDialogOptions, Key, KeyEvent, ScrollSpec, TextEdit, Tx, col, row, text, text_input,
};
use rune_kit::{
    Button, ButtonVariant, Dialog, Kit, Markdown,
    diff::{self, DiffMode, DiffState, DiffView},
    glyphs,
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Conversation,
    Files,
    Changes,
    Worktrees,
    Review,
    Settings,
    Session,
}

pub struct Panels {
    pub screen: Screen,
    pub inspector: Screen,
    pub overlay: Option<Screen>,
    pub searching_sessions: bool,
    pub sessions: TextEdit,
    pub archived: bool,
    search: TextEdit,
    content_search: bool,
    expanded: HashSet<(String, String)>,
    pub review: Option<(String, String)>,
    review_file: Option<String>,
    change_file: Option<(String, String)>,
    diff: DiffState,
    diff_hash: u64,
    change_diff: DiffState,
    change_hash: u64,
    selection: Option<Selection>,
    preview_page: usize,
    branch: TextEdit,
    base: TextEdit,
    directory: TextEdit,
    directory_loaded: bool,
}

struct Selection {
    project: String,
    path: String,
    text: String,
    lines: Option<(usize, usize)>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            screen: Screen::Conversation,
            inspector: Screen::Files,
            overlay: None,
            searching_sessions: false,
            sessions: TextEdit::new(""),
            archived: false,
            search: TextEdit::new(""),
            content_search: false,
            expanded: HashSet::new(),
            review: None,
            review_file: None,
            change_file: None,
            diff: DiffState::new(diff::parse("")),
            diff_hash: 0,
            change_diff: DiffState::new(diff::parse("")),
            change_hash: 0,
            selection: None,
            preview_page: 0,
            branch: TextEdit::new(""),
            base: TextEdit::new(""),
            directory: TextEdit::new(""),
            directory_loaded: false,
        }
    }
}

impl Panels {
    pub fn sync(&mut self, app: &Application) {
        if let Some(patch) = self
            .change_file
            .as_ref()
            .and_then(|(p, f)| app.services.checkouts.get(p)?.diffs.get(f))
            .map(|d| format!("{}{}{}", d.staged, d.unstaged, d.untracked))
        {
            update_diff(&mut self.change_diff, &mut self.change_hash, &patch);
        }
        let patch = if self.screen == Screen::Review {
            self.review
                .as_ref()
                .and_then(|key| app.services.reviews.get(key))
                .and_then(|r| {
                    r.files
                        .iter()
                        .find(|f| Some(&f.path) == self.review_file.as_ref())
                })
                .and_then(|f| {
                    f.patch.as_ref().map(|patch| {
                        format!(
                            "diff --git a/{0} b/{0}\n--- a/{0}\n+++ b/{0}\n{patch}\n",
                            f.path
                        )
                    })
                })
        } else {
            None
        };
        if let Some(patch) = patch {
            update_diff(&mut self.diff, &mut self.diff_hash, &patch);
        }
    }
}

fn update_diff(diff: &mut DiffState, revision: &mut u64, patch: &str) {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    patch.hash(&mut hash);
    let hash = hash.finish();
    if *revision != hash {
        let mode = diff.mode();
        *diff = DiffState::new(diff::parse(patch));
        diff.set_mode(mode);
        *revision = hash;
    }
}

fn label(value: impl Into<String>, k: &Kit) -> El {
    text(value.into()).font(k.ty.ui).color(k.c.text_base).wrap()
}
fn action(label: &str, task: Task, s: &Workspace, k: &Kit) -> El {
    Button::new(label)
        .variant(ButtonVariant::Outline)
        .disabled(live(s).app.services.busy(&task))
        .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
        .build(k)
}
fn scroll(content: El) -> El {
    col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(content)
}

pub fn show(s: &mut Workspace, screen: Screen) {
    if matches!(screen, Screen::Files | Screen::Changes) {
        s.inspector.set_open(true);
    }
    let live = live_mut(s);
    match screen {
        Screen::Files | Screen::Changes => live.panels.inspector = screen,
        Screen::Settings | Screen::Worktrees | Screen::Session => {
            live.panels.overlay = Some(screen)
        }
        _ => live.panels.screen = screen,
    }
    if screen == Screen::Settings && !live.panels.directory_loaded {
        live.panels.directory = TextEdit::new(&live.app.preferences.worktree_directory);
        live.panels.directory_loaded = true;
    }
    let Some(project) = live.app.active_project.clone() else {
        return;
    };
    match screen {
        Screen::Files => live.app.task(Task::Directory {
            project,
            path: String::new(),
            offset: 0,
        }),
        Screen::Changes => live.app.task(Task::Git { project }),
        Screen::Worktrees => live.app.task(Task::Worktrees {
            project,
            directory: live.app.preferences.worktree_directory.clone(),
        }),
        _ => {}
    }
}

pub fn open_link(s: &mut Workspace, index: usize, url: &str, tx: &mut Tx) {
    if url.starts_with("https://") || url.starts_with("http://") {
        let _ = tx.open_url(url);
        return;
    }
    let project = live(s).app.sessions[index].scope.project.clone();
    let (path, line) = url
        .split_once("#L")
        .or_else(|| {
            url.rsplit_once(':')
                .filter(|(_, n)| n.parse::<usize>().is_ok())
        })
        .map_or((url, None), |(path, line)| {
            (
                path,
                line.split('-').next().and_then(|v| v.parse::<usize>().ok()),
            )
        });
    let path = path.strip_prefix("file://").unwrap_or(path);
    let path = std::path::Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(&project).ok()
    } else {
        Some(path)
    };
    if let Some(path) = relative.filter(|p| {
        !p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    }) {
        let live = live_mut(s);
        live.app.select(index);
        live.app
            .open_file(project, path.to_string_lossy().into_owned());
        live.panels.inspector = Screen::Files;
        live.panels.preview_page = line.unwrap_or(1).saturating_sub(1) / 200;
        s.inspector.set_open(true);
    }
}

pub fn navigation(_: &Workspace, k: &Kit) -> El {
    quiet_button(&ui::BRANCH, "Worktrees")
        .on_press(|s: &mut Workspace| show(s, Screen::Worktrees))
        .build(k)
        .key("nav-worktrees")
}

pub fn open_review(s: &mut Workspace, project: String, url: String) {
    let live = live_mut(s);
    live.panels.screen = Screen::Review;
    live.panels.review = Some((project.clone(), url.clone()));
    live.panels.review_file = None;
    live.app.task(Task::Details { project, url });
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, _: &mut Tx) -> bool {
    if ev.is_command('k') {
        s.sidebar.set_open(true);
        live_mut(s).panels.searching_sessions = true;
        live_mut(s).panels.sessions.focused = true;
    } else if ev.is_command('p') {
        show(s, Screen::Files);
        live_mut(s).panels.search.focused = true;
    } else if ev.is_command(',') {
        show(s, Screen::Settings);
    } else if ev.key == Key::Escape && live(s).panels.overlay.is_some() {
        live_mut(s).panels.overlay = None;
    } else if ev.key == Key::Escape && live(s).panels.searching_sessions {
        live_mut(s).panels.sessions.clear();
        live_mut(s).panels.sessions.focused = false;
        live_mut(s).panels.searching_sessions = false;
    } else if ev.key == Key::Escape && live(s).panels.screen != Screen::Conversation {
        live_mut(s).panels.screen = Screen::Conversation;
    } else {
        return false;
    }
    true
}

pub fn view(s: &Workspace, k: &Kit) -> El {
    let mut surface = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(10.0)
        .padding(16.0)
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .child(ui::caption("Pull request", k))
                .child(
                    Button::new("Back to session")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(|s: &mut Workspace| {
                            live_mut(s).panels.screen = Screen::Conversation
                        })
                        .build(k)
                        .key("back-to-session"),
                ),
        );
    if let Some(error) = &live(s).app.persistence_error {
        surface = surface.child(label(error, k));
    }
    if let Some(project) = &live(s).app.active_project
        && let Some(error) = live(s)
            .app
            .services
            .checkouts
            .get(project)
            .and_then(|c| c.error.as_ref())
    {
        surface = surface.child(label(error, k));
    }
    surface.child(review(s, k))
}

pub fn inspector(s: &Workspace, k: &Kit) -> El {
    let mut tabs = row().w_full().h(44.0).px(8.0).center_y().gap(4.0);
    for (name, screen) in [("Files", Screen::Files), ("Changes", Screen::Changes)] {
        let selected = live(s).panels.inspector == screen;
        let tab = Button::new(name)
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| show(s, screen))
            .build(k)
            .h(30.0)
            .key(format!("nav-{}", name.to_lowercase()));
        tabs = tabs.child(if selected {
            tab.fill(k.alpha(k.c.text_accent, 0.09))
        } else {
            tab
        });
    }
    let mut content = col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(k.c.bg_base.mix(k.c.bg_deep, 0.55))
        .border(1.0, k.c.border_base)
        .child(
            tabs.child(rune::el().grow(1.0)).child(
                quiet_button(&glyphs::CLOSE, "Hide inspector")
                    .on_press(|s: &mut Workspace| s.inspector.set_open(false))
                    .build(k)
                    .key("close-inspector"),
            ),
        )
        .child(rune::el().w_full().h(1.0).fill(k.c.border_base));
    if let Some(project) = &live(s).app.active_project
        && let Some(error) = live(s)
            .app
            .services
            .checkouts
            .get(project)
            .and_then(|c| c.error.as_ref())
    {
        content = content.child(ui::caption(error, k).color(k.c.danger.fg).padding(12.0));
    }
    content.child(if live(s).panels.inspector == Screen::Changes {
        changes(s, k)
    } else {
        files(s, k).padding(12.0)
    })
}

pub fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let Some(screen) = live(s).panels.overlay else {
        return;
    };
    let (title, description, body) = match screen {
        Screen::Settings => ("Settings", "Make Caelum feel like home.", settings(s, k)),
        Screen::Worktrees => (
            "Worktrees",
            "Independent checkouts for parallel work.",
            worktrees(s, k),
        ),
        Screen::Session => (
            "Session actions",
            "Manage this session and your provider defaults.",
            scroll(crate::live::session_actions(s, k)),
        ),
        _ => return,
    };
    Dialog::new("workspace", title)
        .description(description)
        .open(true)
        .width(640.0)
        .fit()
        .on_close(|s: &mut Workspace| live_mut(s).panels.overlay = None)
        .child(
            col()
                .w_full()
                .h(420.0)
                .min_h(0.0)
                .px(24.0)
                .pb(24.0)
                .child(body),
        )
        .build(k, layers);
}

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

fn files(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(project) = &live.app.active_project else {
        return label("Open a project first", k);
    };
    let mut browser = col().w_full().gap(6.0).child(
        col()
            .gap(6.0)
            .w_full()
            .child(
                text_input(&live.panels.search, k.ty.ui)
                    .label("Find files or text")
                    .placeholder("Search, then press Enter", k.c.text_muted)
                    .color(k.c.text_base)
                    .key("file-search")
                    .w_full()
                    .min_h(30.0)
                    .padding(6.0)
                    .radius(6.0)
                    .border(1.0, k.c.border_base)
                    .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.search)
                    .on_submit(run_search),
            )
            .child(
                Button::new(if live.panels.content_search {
                    "Content"
                } else {
                    "Paths"
                })
                .variant(ButtonVariant::Outline)
                .on_press(|s: &mut Workspace| {
                    let p = &mut live_mut(s).panels;
                    p.content_search = !p.content_search;
                })
                .build(k),
            )
            .child(
                Button::new("Find")
                    .variant(ButtonVariant::Outline)
                    .on_press_tx(run_search)
                    .build(k)
                    .key("find-files"),
            ),
    );
    let Some(checkout) = live.app.services.checkouts.get(project) else {
        return browser.child(label("Loading files…", k));
    };
    let mut listing = col().w_full().gap(2.0);
    if !live.panels.search.text.is_empty() {
        if let Some(results) = &checkout.search {
            for hit in &results.matches {
                let project = project.clone();
                let path = hit.path.clone();
                let line = hit.line;
                listing = listing.child(
                    Button::new(format!(
                        "{}{} {}",
                        hit.path,
                        hit.line.map(|l| format!(":{l}")).unwrap_or_default(),
                        hit.preview.as_deref().unwrap_or_default()
                    ))
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.panels.preview_page =
                            line.map(|l| (l.saturating_sub(1)) / 200).unwrap_or(0);
                        live.panels.selection = None;
                        live.app.open_file(project.clone(), path.clone());
                    })
                    .build(k),
                );
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
    browser = browser.child(scroll(listing).max_h(180.0));
    let mut tabs = row().gap(4.0).w_full().scroll_x();
    for path in &checkout.tabs {
        let project = project.clone();
        let path = path.clone();
        let close_path = path.clone();
        let close_project = project.clone();
        tabs = tabs
            .child(
                Button::new(&path)
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.panels.preview_page = 0;
                        live.panels.selection = None;
                        live.app.open_file(project.clone(), path.clone());
                    })
                    .build(k),
            )
            .child(
                Button::new("×")
                    .variant(ButtonVariant::GhostMuted)
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
                    .label("Close file"),
            );
    }
    browser = browser.child(tabs);
    let Some(path) = &checkout.selected_file else {
        return browser
            .child(label("Choose a file to preview or attach", k))
            .grow(1.0);
    };
    let Some(file) = checkout.files.get(path) else {
        return browser.child(label("Loading preview…", k)).grow(1.0);
    };
    let project_attach = project.clone();
    let path_attach = path.clone();
    let has_selection = live
        .panels
        .selection
        .as_ref()
        .is_some_and(|selection| &selection.project == project && &selection.path == path);
    let mut actions = col().w_full().gap(6.0).child(
        Button::new(if has_selection {
            "Attach selection"
        } else {
            "Attach file"
        })
        .variant(ButtonVariant::Outline)
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
        .key("attach-preview"),
    );
    let page = live.panels.preview_page;
    let lines: Vec<_> = file.content.lines().collect();
    let start = (page * 200).min(lines.len());
    let end = (start + 200).min(lines.len());
    actions = actions
        .child(
            Button::new("Previous")
                .variant(ButtonVariant::GhostMuted)
                .disabled(page == 0)
                .on_press(|s: &mut Workspace| {
                    let panels = &mut live_mut(s).panels;
                    panels.preview_page = panels.preview_page.saturating_sub(1);
                    panels.selection = None;
                })
                .build(k),
        )
        .child(label(
            format!(
                "Lines {}–{}{}",
                start + 1,
                end,
                if file.truncated { " · truncated" } else { "" }
            ),
            k,
        ))
        .child(
            Button::new("Next")
                .variant(ButtonVariant::GhostMuted)
                .disabled(end == lines.len())
                .on_press(|s: &mut Workspace| {
                    let panels = &mut live_mut(s).panels;
                    panels.preview_page += 1;
                    panels.selection = None;
                })
                .build(k),
        );
    let language = path.rsplit('.').next().unwrap_or_default();
    let preview = lines[start..end].join("\n");
    let fence = "`".repeat(
        preview
            .lines()
            .map(|line| line.trim_start().chars().take_while(|c| *c == '`').count())
            .max()
            .unwrap_or(0)
            .max(3)
            + 1,
    );
    let code = format!("{fence}{language}\n{preview}\n{fence}");
    browser = browser.child(
        action(
            "Refresh preview",
            Task::File {
                project: project.clone(),
                path: path.clone(),
            },
            s,
            k,
        )
        .self_start(),
    );
    let project = project.clone();
    let path = path.clone();
    browser
        .child(actions)
        .child(scroll(
            Markdown::new(&code)
                .selectable(true)
                .render(&theme::markdown(k))
                .on_selection_change(move |s: &mut Workspace, change, _| {
                    live_mut(s).panels.selection = change.selection().map(|selection| {
                        let range = selection.texts().first().map(|text| text.range());
                        let lines = range
                            .filter(|range| preview.get(range.clone()) == Some(selection.text()))
                            .map(|range| {
                                let first = start
                                    + 1
                                    + preview[..range.start]
                                        .bytes()
                                        .filter(|b| *b == b'\n')
                                        .count();
                                (
                                    first,
                                    first + selection.text().lines().count().saturating_sub(1),
                                )
                            });
                        Selection {
                            project: project.clone(),
                            path: path.clone(),
                            text: selection.text().to_owned(),
                            lines,
                        }
                    });
                }),
        ))
        .grow(1.0)
        .min_h(0.0)
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
        return tree;
    };
    for entry in &listing.entries {
        let dir = entry.kind == ProjectEntryKind::Directory;
        let p = project.to_owned();
        let f = entry.path.clone();
        let expanded = live(s).panels.expanded.contains(&(p.clone(), f.clone()));
        tree = tree.child(
            Button::new(format!(
                "{}{} {}",
                "  ".repeat(depth),
                if dir {
                    if expanded { "▾" } else { "▸" }
                } else {
                    "·"
                },
                entry.name
            ))
            .icon(if dir { &ui::FOLDER } else { &ui::FILE })
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let live = live_mut(s);
                if dir {
                    if !live.panels.expanded.remove(&(p.clone(), f.clone())) {
                        live.panels.expanded.insert((p.clone(), f.clone()));
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
            .build(k)
            .key(format!("file-tree-{}", entry.path)),
        );
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

fn diff_view(s: &Workspace, k: &Kit, review: bool) -> El {
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
                        &mut live_mut(s).panels.diff
                    } else {
                        &mut live_mut(s).panels.change_diff
                    }
                },
            )
            .build(s, k),
        )
}

fn changes(s: &Workspace, k: &Kit) -> El {
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
    }
    if live(s)
        .panels
        .change_file
        .as_ref()
        .is_some_and(|(p, _)| p == project)
    {
        content = content.child(diff_view(s, k, false));
    }
    content
}

pub fn select_change(s: &mut Workspace, project: String, path: String) {
    let live = live_mut(s);
    live.panels.change_file = Some((project.clone(), path.clone()));
    live.panels.change_diff = DiffState::new(diff::parse(""));
    live.panels.change_hash = 0;
    live.app.task(Task::Diff { project, path });
}

impl crate::live::Live {
    fn diff_reset(&mut self) {
        self.panels.diff = DiffState::new(diff::parse(""));
        self.panels.diff_hash = 0;
    }
}

fn worktrees(s: &Workspace, k: &Kit) -> El {
    let live = live(s);
    let Some(project) = &live.app.active_project else {
        return label("Open a project first", k);
    };
    let mut content = col().w_full().gap(12.0);
    if let Some(listing) = live
        .app
        .services
        .checkouts
        .get(project)
        .and_then(|c| c.worktrees.as_ref())
    {
        content = content.child(label(format!("Storage: {}", listing.storage_directory), k));
        for tree in &listing.worktrees {
            let path = tree.project.path.clone();
            let mut line = col()
                .w_full()
                .gap(8.0)
                .padding(12.0)
                .radius(8.0)
                .border(1.0, k.c.border_base)
                .child(
                    Button::new(format!(
                        "{} · {}",
                        tree.branch.as_deref().unwrap_or("detached"),
                        tree.project.path
                    ))
                    .variant(ButtonVariant::GhostMuted)
                    .disabled(!tree.available)
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        live.app.open_project(path.clone());
                        live.panels.overlay = None;
                    })
                    .build(k),
                );
            if !tree.main && tree.locked.is_none() {
                line = line.child(action(
                    "Remove checkout",
                    Task::RemoveWorktree {
                        project: project.clone(),
                        path: tree.path.clone(),
                    },
                    s,
                    k,
                ));
            }
            content = content.child(line);
        }
        let default = listing.default_base.clone();
        content = content
            .child(label("New worktree", k))
            .child(
                text_input(&live.panels.branch, k.ty.ui)
                    .label("New branch")
                    .placeholder("New branch name", k.c.text_muted)
                    .color(k.c.text_base)
                    .w_full()
                    .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.branch)
                    .key("worktree-branch"),
            )
            .child(
                text_input(&live.panels.base, k.ty.ui)
                    .label("Base branch")
                    .placeholder(&default, k.c.text_muted)
                    .color(k.c.text_base)
                    .w_full()
                    .on_edit(|s: &mut Workspace| &mut live_mut(s).panels.base)
                    .key("worktree-base"),
            );
        let project = project.clone();
        content = content.child(
            Button::new("Create worktree")
                .variant(ButtonVariant::Outline)
                .disabled(live.panels.branch.text.trim().is_empty())
                .on_press(move |s: &mut Workspace| {
                    let live = live_mut(s);
                    live.app.task(Task::CreateWorktree {
                        project: project.clone(),
                        branch: live.panels.branch.text.clone(),
                        base: if live.panels.base.text.trim().is_empty() {
                            default.clone()
                        } else {
                            live.panels.base.text.clone()
                        },
                        directory: live.app.preferences.worktree_directory.clone(),
                    });
                })
                .build(k)
                .key("create-worktree"),
        );
    } else {
        content = content.child(label("No Git worktrees available", k));
    }
    scroll(content)
}

fn settings(s: &Workspace, k: &Kit) -> El {
    let preferences = &live(s).app.preferences;
    let mut content = col().w_full().gap(16.0).child(ui::section(
        "Appearance",
        "Choose a theme and an accent for your workspace.",
        k,
    ));
    let mut themes = row().w_full().gap(8.0);
    for mode in ["system", "dark", "light"] {
        themes = themes.child(
            Button::new(mode)
                .variant(if preferences.theme == mode {
                    ButtonVariant::Outline
                } else {
                    ButtonVariant::GhostMuted
                })
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.preferences.theme = mode.into();
                    s.dark = None;
                })
                .build(k)
                .key(format!("theme-{mode}"))
                .grow(1.0),
        );
    }
    content = content.child(themes);
    let mut families = row().w_full().gap(8.0);
    for (id, name) in theme::FAMILIES {
        let palette = theme::appearance(true, id, "theme");
        families = families.child(
            k.interactive(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(8.0)
                    .padding(10.0)
                    .radius(8.0)
                    .fill(k.c.bg_base)
                    .border(
                        1.0,
                        if preferences.theme_family == id {
                            k.c.border_focus
                        } else {
                            k.c.border_base
                        },
                    )
                    .child(
                        row()
                            .w_full()
                            .h(30.0)
                            .radius(5.0)
                            .fill(palette.c.bg_base)
                            .child(rune::el().w(22.0).h_full().fill(palette.c.bg_deep))
                            .child(
                                rune::el()
                                    .w(30.0)
                                    .h(5.0)
                                    .ml(8.0)
                                    .mt(12.0)
                                    .radius(2.0)
                                    .fill(palette.c.text_accent),
                            ),
                    )
                    .child(ui::caption(name, k)),
            )
            .role("button")
            .label(name)
            .focusable(true)
            .focus_ring(1.0, 0.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| live_mut(s).app.preferences.theme_family = id.into())
            .key(format!("family-{id}")),
        );
    }
    content = content
        .child(families)
        .child(ui::caption("Accent colour", k));
    let mut accents = row().gap(8.0);
    for accent in theme::ACCENTS {
        let palette = theme::appearance(true, &preferences.theme_family, accent);
        accents = accents.child(
            k.interactive(
                rune::el()
                    .size(30.0, 30.0)
                    .radius(15.0)
                    .fill(palette.c.text_accent)
                    .border(
                        if preferences.accent_color == accent {
                            3.0
                        } else {
                            1.0
                        },
                        k.c.text_base,
                    ),
            )
            .role("button")
            .label(format!("{accent} accent"))
            .focusable(true)
            .focus_ring(2.0, 2.0, k.c.border_focus)
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.preferences.accent_color = accent.into()
            })
            .key(format!("accent-{accent}")),
        );
    }
    content = content
        .child(accents)
        .child(ui::section(
            "Workspace",
            "Defaults are saved on this Mac.",
            k,
        ))
        .child(
            Button::new(if live(s).app.preferences.notifications {
                "Notifications: on"
            } else {
                "Notifications: off"
            })
            .variant(ButtonVariant::Outline)
            .on_press(|s: &mut Workspace| {
                let preferences = &mut live_mut(s).app.preferences;
                preferences.notifications = !preferences.notifications;
            })
            .build(k),
        );
    content = content.child(label("Worktree storage directory", k)).child(text_input(&live(s).panels.directory, k.ty.ui).label("Worktree storage directory").placeholder("~/.caelum/worktrees", k.c.text_muted).color(k.c.text_base).w_full().padding(8.0).radius(6.0).border(1.0, k.c.border_base).on_edit(|s: &mut Workspace| &mut live_mut(s).panels.directory))
        .child(Button::new("Save directory").variant(ButtonVariant::Outline).on_press(|s: &mut Workspace| { let live = live_mut(s); live.app.preferences.worktree_directory = live.panels.directory.text.clone(); }).build(k))
        .child(Button::new("Import legacy v1 settings JSON").variant(ButtonVariant::Outline).on_press_tx(|_: &mut Workspace, tx| { tx.open_file(FileDialogOptions { title: "Import exported caelum.settings.v1 JSON".into(), ..Default::default() }, |s: &mut Workspace, result, _| { match result { Ok(Some(path)) => live_mut(s).app.task(Task::ImportSettings { path: path.to_string_lossy().into_owned() }), Ok(None) => {}, Err(error) => live_mut(s).app.error = Some(error.to_string()) } }); }).build(k).key("import-settings"))
        .child(label("Export in Tauri’s Web Inspector: copy(localStorage.getItem('caelum.settings.v1')), save as JSON, then import here. Provider defaults and favourites can also be saved from Session options.", k));
    if let Some(error) = live(s)
        .app
        .services
        .checkouts
        .get("")
        .and_then(|c| c.error.as_ref())
    {
        content = content.child(label(error, k));
    }
    if let Some(error) = &live(s).app.persistence_error {
        content = content.child(label(error, k)).child(
            Button::new("Retry save")
                .variant(ButtonVariant::Outline)
                .on_press(|s: &mut Workspace| live_mut(s).app.retry_save())
                .build(k),
        );
    }
    scroll(content)
}

fn review(s: &Workspace, k: &Kit) -> El {
    let Some((project, url)) = &live(s).panels.review else {
        return label("Choose a pull request", k);
    };
    let Some(review) = live(s)
        .app
        .services
        .reviews
        .get(&(project.clone(), url.clone()))
    else {
        return label("Loading pull request…", k);
    };
    let mut content = col().w_full().gap(12.0);
    if let Some(error) = &review.error {
        content = content.child(label(error, k));
    }
    if let Some(notice) = &review.notice {
        content = content.child(label(notice, k));
    }
    let Some(details) = &review.details else {
        return scroll(content.child(label("Loading overview…", k)));
    };
    let head = &details.head_ref_oid;
    let link = url.clone();
    let refresh_project = project.clone();
    let refresh_url = url.clone();
    content = content
        .child(ui::section(
            &format!(
                "#{} {}{}",
                details.pull_request.number,
                details.pull_request.title,
                if details.pull_request.is_draft {
                    " · Draft"
                } else {
                    ""
                }
            ),
            &format!(
                "{} → {} · {} · +{} −{} · {} files",
                details.head_ref_name,
                details.pull_request.base_ref_name,
                details.state,
                details.additions,
                details.deletions,
                details.changed_files
            ),
            k,
        ))
        .child(
            row()
                .gap(6.0)
                .child(
                    Button::new("GitHub")
                        .variant(ButtonVariant::Outline)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&link);
                        })
                        .build(k),
                )
                .child(
                    Button::new("Refresh review")
                        .variant(ButtonVariant::Outline)
                        .on_press(move |s: &mut Workspace| {
                            let live = live_mut(s);
                            live.app
                                .services
                                .refresh_review(&refresh_project, &refresh_url);
                            live.app.task(Task::Details {
                                project: refresh_project.clone(),
                                url: refresh_url.clone(),
                            });
                        })
                        .build(k),
                ),
        )
        .child(
            Markdown::new(&details.body)
                .selectable(true)
                .render(&theme::markdown(k)),
        );
    for check in details.status_check_rollup.iter().flatten() {
        let (name, status) = match check {
            caelum_protocol::GitHubStatusCheck::CheckRun {
                name,
                status,
                conclusion,
                ..
            } => (name, conclusion.as_ref().unwrap_or(status)),
            caelum_protocol::GitHubStatusCheck::StatusContext { context, state, .. } => {
                (context, state)
            }
        };
        let color = match status.as_str() {
            "SUCCESS" | "success" => k.c.success.fg,
            "FAILURE" | "failure" | "ERROR" | "error" => k.c.danger.fg,
            _ => k.c.text_muted,
        };
        content = content.child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .py(4.0)
                .child(rune::el().size(6.0, 6.0).radius(3.0).fill(color))
                .child(label(name, k).grow(1.0))
                .child(ui::caption(status, k).color(color)),
        );
    }
    if details.state == "OPEN" {
        let mut mutations = row().w_full().scroll_x().gap(6.0).child(action(
            "Update branch",
            Task::UpdateBranch {
                project: project.clone(),
                url: url.clone(),
                head: head.clone(),
            },
            s,
            k,
        ));
        if !details.pull_request.is_draft {
            for (name, method) in [
                ("Squash merge", GitHubMergeMethod::Squash),
                ("Merge", GitHubMergeMethod::Merge),
                ("Rebase merge", GitHubMergeMethod::Rebase),
            ] {
                mutations = mutations.child(action(
                    name,
                    Task::Merge {
                        project: project.clone(),
                        url: url.clone(),
                        head: head.clone(),
                        method,
                    },
                    s,
                    k,
                ));
            }
        }
        content = content.child(mutations);
    }
    content = content.child(ui::section(
        "Changed files",
        "Select a file to review its patch and feedback.",
        k,
    ));
    let owner = (project.clone(), url.clone());
    let lookup = owner.clone();
    let kit = k.clone();
    content = content.child(
        col()
            .key("review-file-list")
            .w_full()
            .h(180.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(
                col()
                    .w_full()
                    .growth(rune::GrowthSpec::default())
                    .virtual_rows(
                        review.files.len(),
                        32.0,
                        move |s: &Workspace, i| {
                            format!(
                                "review-file-{}",
                                live(s).app.services.reviews[&owner].files[i].path
                            )
                            .into()
                        },
                        move |s: &Workspace, i| {
                            let file = &live(s).app.services.reviews[&lookup].files[i];
                            let path = file.path.clone();
                            Button::new(format!(
                                "{} +{} −{}",
                                file.path, file.additions, file.deletions
                            ))
                            .variant(ButtonVariant::GhostMuted)
                            .on_press(move |s: &mut Workspace| {
                                live_mut(s).panels.review_file = Some(path.clone());
                                live_mut(s).diff_reset();
                            })
                            .build(&kit)
                            .w_full()
                        },
                    ),
            ),
    );
    if let Some(file) = review
        .files
        .iter()
        .find(|file| live(s).panels.review_file.as_ref() == Some(&file.path))
    {
        content = content.child(label(&file.path, k));
        if file.patch.is_none() {
            content = content.child(label("Patch unavailable (binary, truncated or too large). Open GitHub for the complete file.", k));
        } else {
            content = content.child(diff_view(s, k, true).h(400.0).min_h(400.0));
        }
        for thread in review.threads.iter().filter(|t| t.path == file.path) {
            content = content.child(thread_view(s, k, project, url, thread));
        }
    }
    if let Some(page) = review.next_page {
        content = content.child(action(
            "Load more files",
            Task::ReviewFiles {
                project: project.clone(),
                url: url.clone(),
                head: head.clone(),
                page,
            },
            s,
            k,
        ));
    }
    content = content.child(label("Feedback", k));
    for comment in review.discussion.iter().chain(&review.reviews) {
        content = content.child(comment_view(s, k, project, url, comment));
    }
    for thread in &review.threads {
        if live(s).panels.review_file.as_ref() != Some(&thread.path) {
            content = content.child(thread_view(s, k, project, url, thread));
        }
    }
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
    let key = (project.clone(), url.clone());
    content = content.child(
        Button::new(format!(
            "Address {} selected comments",
            review.selected_comments.len()
        ))
        .variant(ButtonVariant::Outline)
        .disabled(
            review.selected_comments.is_empty()
                || live(s)
                    .app
                    .selected
                    .is_none_or(|i| live(s).app.sessions[i].scope.project != *project),
        )
        .on_press(|s: &mut Workspace| {
            let live = live_mut(s);
            let Some(index) = live.app.selected else {
                return;
            };
            if let Some(prompt) = live
                .panels
                .review
                .as_ref()
                .and_then(|key| live.app.services.reviews.get(key))
                .and_then(|r| r.address_prompt())
            {
                if !live.drafts[index].text.is_empty() {
                    live.drafts[index].text.push_str("\n\n");
                }
                live.drafts[index].text.push_str(&prompt);
                live.panels.screen = Screen::Conversation;
                live.drafts[index].focused = true;
            }
        })
        .build(k)
        .key(format!("address-comments-{}", key.1)),
    );
    scroll(content)
}

fn comment_view(
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
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(10.0)
        .fill(k.c.layer_02)
        .border(1.0, k.c.border_base)
        .child(
            Button::new(format!(
                "{} {} · {}",
                if selected { "✓" } else { "○" },
                comment
                    .author
                    .as_ref()
                    .map(|a| a.login.as_str())
                    .unwrap_or("Unknown"),
                comment.state.as_deref().unwrap_or("Comment")
            ))
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                let review = live_mut(s).app.services.reviews.get_mut(&key).unwrap();
                if !review.selected_comments.remove(&id) {
                    review.selected_comments.insert(id.clone());
                }
            })
            .build(k),
        )
        .child(
            Markdown::new(&comment.body)
                .selectable(true)
                .render(&theme::markdown(k)),
        )
}
fn thread_view(
    s: &Workspace,
    k: &Kit,
    project: &str,
    url: &str,
    thread: &caelum_protocol::GitHubReviewThread,
) -> El {
    let mut content = col().w_full().gap(6.0).child(label(
        format!(
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
        ),
        k,
    ));
    if let Some(hunk) = thread.comments.first().and_then(|c| c.diff_hunk.as_ref()) {
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

pub fn attach_dialog(s: &mut Workspace, tx: &mut Tx) {
    let Some(index) = live(s).app.selected else {
        return;
    };
    let project = live(s).app.sessions[index].scope.project.clone();
    tx.open_file(
        FileDialogOptions {
            title: "Attach a project file".into(),
            ..Default::default()
        },
        move |s: &mut Workspace, result, _| match result {
            Ok(Some(path)) => match path.strip_prefix(&project) {
                Ok(path) => live_mut(s)
                    .app
                    .attach_file(index, path.to_string_lossy().into_owned()),
                Err(_) => {
                    live_mut(s).app.sessions[index].error =
                        Some("Choose a file inside this session’s project".into())
                }
            },
            Ok(None) => {}
            Err(error) => live_mut(s).app.sessions[index].error = Some(error.to_string()),
        },
    );
}

pub fn composer_context(s: &Workspace, k: &Kit, index: usize) -> El {
    let session = &live(s).app.sessions[index];
    let mut context = col().w_full().gap(4.0).max_h(180.0).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if let Some(error) = &live(s).app.persistence_error {
        context = context.child(label(error, k));
    }
    for attachment in &session.attachments {
        let id = attachment.id.clone();
        context = context.child(
            Button::new(format!(
                "{}{} ×",
                attachment.path,
                if attachment.truncated {
                    " (truncated)"
                } else {
                    ""
                }
            ))
            .variant(ButtonVariant::GhostMuted)
            .on_press(move |s: &mut Workspace| {
                live_mut(s).app.sessions[index]
                    .attachments
                    .retain(|f| f.id != id)
            })
            .build(k),
        );
    }
    for (position, queued) in session.queue.iter().enumerate() {
        context = context.child(
            row()
                .gap(4.0)
                .child(label(
                    format!("Queued: {}", queued.text.lines().next().unwrap_or_default()),
                    k,
                ))
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
                        .build(k),
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
                        .build(k),
                ),
        );
    }
    if session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Resume queued messages")
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut Workspace| live_mut(s).app.resume_queue(index))
                .build(k),
        );
    }
    if !session.queue_paused && !session.queue.is_empty() {
        context = context.child(
            Button::new("Pause queue")
                .variant(ButtonVariant::GhostMuted)
                .on_press(move |s: &mut Workspace| {
                    live_mut(s).app.sessions[index].queue_paused = true
                })
                .build(k),
        );
    }
    if session.attaching > 0 {
        context = context.child(label("Reading attachments…", k));
    }
    if !session.shells.is_empty() {
        context = context.child(label(
            format!("{} background shell(s) running", session.shells.len()),
            k,
        ));
    }
    if let Some(used) = session.conversation.used_tokens {
        let limit = session.conversation.context_window.or_else(|| {
            live(s)
                .app
                .choices
                .get(&session.scope)?
                .models
                .iter()
                .find(|m| Some(&m.id) == session.options.model.as_ref())?
                .context_window
        });
        context = context.child(label(
            format!(
                "Context: {used}{} tokens",
                limit.map(|max| format!(" / {max}")).unwrap_or_default()
            ),
            k,
        ));
    }
    let draft = &live(s).drafts[index].text;
    if let Some((start, query)) = mention(draft)
        && let Some(checkout) = live(s).app.services.checkouts.get(&session.scope.project)
        && checkout.completion_query == query
        && let Some(results) = &checkout.completions
    {
        for hit in results.matches.iter().take(8) {
            let path = hit.path.clone();
            let expected = draft.clone();
            context = context.child(
                Button::new(format!("Attach @{}", path))
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        let live = live_mut(s);
                        if live.drafts[index].text == expected {
                            live.drafts[index].text.truncate(start);
                            live.app.attach_file(index, path.clone());
                            live.drafts[index].focused = true;
                        }
                    })
                    .build(k),
            );
        }
    }
    if draft.starts_with('/') && !draft.contains(char::is_whitespace) {
        let mut commands: Vec<String> = ["new", "files", "changes", "settings", "ask"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        if let Some(provider) = live(s).app.provider_commands.get(&session.scope) {
            commands.extend(provider.iter().map(|c| c.name.clone()));
        }
        for command in commands
            .into_iter()
            .filter(|c| c.starts_with(&draft[1..]))
            .take(8)
        {
            context = context.child(
                Button::new(format!("/{command}"))
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(move |s: &mut Workspace| {
                        live_mut(s).drafts[index] = TextEdit::new(format!("/{command} "));
                        live_mut(s).drafts[index].focused = true;
                    })
                    .build(k),
            );
        }
    }
    context
}

pub fn mention(text: &str) -> Option<(usize, &str)> {
    let start = text
        .rfind(char::is_whitespace)
        .map_or(0, |i| i + text[i..].chars().next().unwrap().len_utf8());
    text[start..].strip_prefix('@').map(|query| (start, query))
}

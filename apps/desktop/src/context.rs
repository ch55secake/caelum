//! App-owned context menus. Actions and editor selections belong to the clicked object.
use std::rc::Rc;

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels, ui,
};
use caelum_core::application::{Attachment, Task};
use rune::{El, Icon, Key, KeyEvent, PointerPress, TextEdit, Tx};
use rune_kit::{Kit, MenuItem, glyphs};

type Choose = Box<dyn FnOnce(&mut Workspace, &str, &mut Tx)>;

#[derive(Default)]
pub struct State {
    opened: Option<Opened>,
    revision: u64,
    paste: Option<(String, Scope, bool)>,
}

struct Opened {
    point: PointerPress,
    origin: String,
    scope: Scope,
    menu: Menu,
    choose: Choose,
}

#[derive(PartialEq, Eq)]
struct Scope {
    session: Option<usize>,
    project: Option<String>,
    screen: panels::Screen,
    inspector: panels::Screen,
    overlay: Option<panels::Screen>,
    settings: crate::settings::Page,
    models: bool,
    side: Option<usize>,
    question: Option<String>,
    file: Option<String>,
    review: Option<(String, String)>,
    background: bool,
}

impl Scope {
    fn of(l: &Live) -> Self {
        Self {
            session: l.app.selected,
            project: l.app.active_project.clone(),
            screen: l.panels.screen,
            inspector: l.panels.inspector,
            overlay: l.panels.overlay,
            settings: l.panels.settings_page,
            models: l.models.is_open(),
            side: l.commands.side_owner,
            question: l
                .app
                .selected
                .and_then(|index| crate::questions::active(l, index).map(|p| p.request_id.clone())),
            file: l
                .app
                .active_project
                .as_ref()
                .and_then(|p| l.app.services.checkouts.get(p))
                .and_then(|c| c.selected_file.clone()),
            review: l.panels.review.clone(),
            background: l.app.window_background,
        }
    }
}

struct Item {
    id: &'static str,
    title: &'static str,
    enabled: bool,
    icon: Option<&'static Icon>,
    shortcut: Option<&'static str>,
}

impl Item {
    fn new(id: &'static str, title: &'static str) -> Self {
        Self {
            id,
            title,
            enabled: true,
            icon: None,
            shortcut: None,
        }
    }
    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    fn icon(mut self, icon: &'static Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    fn shortcut(mut self, shortcut: &'static str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
}

#[derive(Default)]
struct Menu(Vec<Option<Item>>);
impl Menu {
    fn new() -> Self {
        Self::default()
    }
    fn item(mut self, item: Item) -> Self {
        self.0.push(Some(item));
        self
    }
    fn separator(mut self) -> Self {
        self.0.push(None);
        self
    }
}

fn show(
    s: &mut Workspace,
    point: PointerPress,
    origin: String,
    menu: Menu,
    choose: impl FnOnce(&mut Workspace, &str, &mut Tx) + 'static,
) {
    let l = live_mut(s);
    l.context.revision += 1;
    l.context.paste = None;
    l.context.opened = Some(Opened {
        point,
        origin,
        scope: Scope::of(l),
        menu,
        choose: Box::new(choose),
    });
}

pub fn is_open(s: &Workspace) -> bool {
    s.live.as_ref().is_some_and(|l| {
        l.context
            .opened
            .as_ref()
            .is_some_and(|m| m.scope == Scope::of(l))
    })
}

pub fn close(s: &mut Workspace) {
    let state = &mut live_mut(s).context;
    state.opened = None;
    state.paste = None;
    state.revision += 1;
}

pub fn sync(l: &mut Live, tx: &mut Tx) -> bool {
    if let Some((origin, scope, waited)) = l.context.paste.take()
        && scope == Scope::of(l)
    {
        // Clipboard requests capture the presented input, so wait for the menu's
        // closing commit to return focus before using Rune's guarded paste route.
        if l.focus_key.as_deref() == Some(&origin) {
            tx.request_paste();
        } else if !waited {
            l.context.paste = Some((origin, scope, true));
            tx.wake_after(std::time::Duration::ZERO);
        }
    }
    if l.context
        .opened
        .as_ref()
        .is_some_and(|m| m.scope != Scope::of(l))
    {
        l.context.opened = None;
        l.context.revision += 1;
        return true;
    }
    false
}

pub fn key(s: &mut Workspace, event: &KeyEvent) -> bool {
    if matches!(event.key, Key::Escape | Key::Tab) {
        close(s);
        return true;
    }
    false
}

pub fn overlay(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    if !is_open(s) {
        return;
    }
    let state = &live(s).context;
    let opened = state.opened.as_ref().unwrap();
    let revision = state.revision;
    let icons = opened
        .menu
        .0
        .iter()
        .flatten()
        .any(|item| item.icon.is_some());
    layers.push(
        rune::el()
            .key("context-scrim")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .on_press(close)
            .on_secondary_press(|s: &mut Workspace, _, _| close(s)),
    );
    let mut menu = rune_kit::Menu::new("context")
        .open(true)
        .at(opened.point.x, opened.point.y)
        .min_width(232.0);
    for entry in &opened.menu.0 {
        let Some(item) = entry else {
            menu = menu.separator();
            continue;
        };
        let id = item.id;
        let mut row = MenuItem::new(item.title)
            .disabled(!item.enabled)
            .on_select_tx(move |s: &mut Workspace, tx| {
                if live(s).context.revision != revision || !is_open(s) {
                    return;
                }
                let opened = live_mut(s).context.opened.take().unwrap();
                close(s);
                (opened.choose)(s, id, tx);
            });
        if let Some(icon) = item.icon {
            row = row.icon(icon);
        } else if icons {
            row = row.lead(rune::el().size(14.0, 14.0).a11y_hidden());
        }
        if let Some(shortcut) = item.shortcut {
            row = row.shortcut(shortcut);
        }
        menu = menu.item(row);
    }
    // The source is already mounted in a virtual row or pane. Return focus there,
    // rather than to the synthetic trigger that Menu normally wraps.
    menu.build(k, layers);
    let surface = layers
        .pop()
        .unwrap()
        .label("Context menu")
        .modal(true, close)
        .on_key(|s: &mut Workspace, event, _| key(s, event))
        .focus_return(&opened.origin);
    layers.push(surface);
}

fn selection(tx: &Tx, origin: &str) -> Option<String> {
    tx.text_selection()
        .filter(|selection| {
            selection
                .texts()
                .iter()
                .any(|text| text.path().split('/').any(|part| part == origin) && text.is_selected())
        })
        .map(|selection| selection.text().to_owned())
}

pub fn message(el: El, text: String, index: usize, origin: String) -> El {
    el.focusable(true)
        .tab_index(-1)
        .on_secondary_press(move |s: &mut Workspace, at, tx| {
            let text = text.clone();
            let selected = selection(tx, &origin);
            let mut menu = Menu::new();
            if selected.is_some() {
                menu = menu
                    .item(Item::new("selection", "Copy selection").icon(&glyphs::COPY))
                    .separator();
            }
            menu = menu
                .item(Item::new("copy", "Copy message").icon(&glyphs::COPY))
                .item(
                    Item::new("reuse", "Use in prompt")
                        .icon(&ui::EDIT)
                        .enabled(live(s).app.selected == Some(index)),
                );
            show(s, at, origin.clone(), menu, move |s, pick, tx| match pick {
                "selection" => tx.copy(selected.unwrap_or_default()),
                "copy" => tx.copy(text.clone()),
                "reuse" if live(s).app.selected == Some(index) => {
                    let l = live_mut(s);
                    let draft = &mut l.drafts[index];
                    draft.set_caret(draft.text.len());
                    draft.insert(&if draft.text.is_empty() {
                        text
                    } else {
                        format!("\n\n{text}")
                    });
                    draft.focused = true;
                    crate::completion::changed(s, tx);
                }
                _ => {}
            });
        })
}

pub fn file(
    el: El,
    project: String,
    path: String,
    content: String,
    range: Option<(usize, usize)>,
    origin: String,
) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let owner = live(s).app.selected;
        let project = project.clone();
        let path = path.clone();
        let content = content.clone();
        let menu = Menu::new()
            .item(
                Item::new(
                    "copy",
                    if range.is_some() {
                        "Copy selection"
                    } else {
                        "Copy content"
                    },
                )
                .icon(&glyphs::COPY),
            )
            .item(Item::new("path", "Copy relative path"))
            .item(Item::new("full-path", "Copy full path"))
            .separator()
            .item(
                Item::new("attach", "Add to prompt")
                    .icon(&ui::FILE)
                    .enabled(
                        owner.is_some_and(|i| live(s).app.sessions[i].scope.project == project),
                    ),
            )
            .item(Item::new("reveal", "Reveal in Finder").icon(&ui::FOLDER));
        show(s, at, origin.clone(), menu, move |s, pick, tx| match pick {
            "copy" => tx.copy(content.clone()),
            "path" => tx.copy(path.clone()),
            "full-path" => tx.copy(std::path::Path::new(&project).join(&path).to_string_lossy()),
            "attach" if live(s).app.selected == owner => {
                if let Some(i) = owner.filter(|&i| live(s).app.sessions[i].scope.project == project)
                {
                    live_mut(s).app.attach(
                        i,
                        Attachment::new(path.clone(), content.clone(), range, false),
                    );
                }
            }
            "reveal" => live_mut(s).app.task(Task::Reveal {
                project: project.clone(),
                path: path.clone(),
            }),
            _ => {}
        });
    })
}

pub fn project(el: El, project: String) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let project = project.clone();
        let origin = crate::code_view::source_key(&project, "project");
        let menu = Menu::new()
            .item(Item::new("new", "New session").icon(&crate::PLUS))
            .item(Item::new("worktree", "Create worktree…").icon(&ui::BRANCH))
            .separator()
            .item(Item::new("copy", "Copy project path").icon(&glyphs::COPY))
            .item(Item::new("reveal", "Reveal in Finder").icon(&ui::FOLDER))
            .separator()
            .item(Item::new("close", "Close project").icon(&glyphs::CLOSE));
        show(s, at, origin, menu, move |s, pick, tx| match pick {
            "copy" => tx.copy(project.clone()),
            "close" => live_mut(s).close_project(&project),
            "reveal" => live_mut(s).app.task(Task::Reveal {
                project: project.clone(),
                path: ".".into(),
            }),
            "new" => {
                let l = live_mut(s);
                l.app.select_project(&project);
                l.new_session();
            }
            "worktree" => {
                let provider = live(s).app.preferences.default_provider;
                crate::worktree::open(s, project.clone(), provider);
            }
            _ => {}
        });
    })
}

pub fn worktree(el: El, project: String, path: String, origin: String) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let app = &live(s).app;
        let Some(tree) = app
            .managed_worktrees(&project)
            .find(|tree| tree.path == path)
        else {
            return;
        };
        let directory = tree.project.path.clone();
        let project = project.clone();
        let path = path.clone();
        let current_name = crate::checkouts::name(tree).to_owned();
        let rename = Task::RenameWorktree {
            project: project.clone(),
            path: path.clone(),
            name: current_name.clone(),
            directory: app.preferences.worktree_directory.clone(),
        };
        let menu = Menu::new()
            .item(Item::new("copy", "Copy worktree path").icon(&glyphs::COPY))
            .item(
                Item::new("reveal", "Reveal in Finder")
                    .icon(&ui::FOLDER)
                    .enabled(tree.available),
            )
            .item(
                Item::new("rename", "Rename worktree")
                    .icon(&ui::EDIT)
                    .enabled(!app.services.busy(&rename)),
            )
            .separator()
            .item(
                Item::new("delete", "Delete worktree")
                    .icon(&glyphs::CLOSE)
                    .enabled(crate::checkouts::removal(app, &project, &path).is_some()),
            );
        show(s, at, origin.clone(), menu, move |s, pick, tx| match pick {
            "copy" => tx.copy(path),
            "reveal" => live_mut(s).app.task(Task::Reveal {
                project: directory,
                path: ".".into(),
            }),
            "rename" => {
                crate::checkouts::manage(s, project.clone());
                crate::worktree::begin_rename(
                    s,
                    project.clone(),
                    path.clone(),
                    current_name.clone(),
                );
            }
            "delete" => {
                let app = &mut live_mut(s).app;
                if let Some(task) = crate::checkouts::removal(app, &project, &path) {
                    app.task(task);
                }
            }
            _ => {}
        });
    })
}

pub fn path(el: El, project: String, path: String, directory: bool, origin: String) -> El {
    el.key(&origin)
        .on_secondary_press(move |s: &mut Workspace, at, _| {
            let owner = live(s).app.selected;
            let project = project.clone();
            let path = path.clone();
            let opened = live(s)
                .app
                .services
                .checkouts
                .get(&project)
                .is_some_and(|c| c.tabs.contains(&path));
            let mut menu = Menu::new()
                .item(
                    Item::new(
                        "open",
                        if directory {
                            "Open folder"
                        } else {
                            "Open file"
                        },
                    )
                    .icon(if directory { &ui::FOLDER } else { &ui::FILE }),
                )
                .separator()
                .item(Item::new("relative", "Copy relative path").icon(&glyphs::COPY))
                .item(Item::new("full", "Copy full path"))
                .item(Item::new("reveal", "Reveal in Finder").icon(&ui::FOLDER));
            if !directory {
                menu = menu.separator().item(
                    Item::new("attach", "Add to prompt")
                        .icon(&ui::EDIT)
                        .enabled(
                            owner.is_some_and(|i| live(s).app.sessions[i].scope.project == project),
                        ),
                );
            }
            if opened {
                menu = menu.item(Item::new("close", "Close file").icon(&glyphs::CLOSE));
            }
            show(s, at, origin.clone(), menu, move |s, pick, tx| match pick {
                "open" if directory => {
                    let l = live_mut(s);
                    l.app
                        .preferences
                        .files
                        .entry(project.clone())
                        .or_default()
                        .expanded
                        .insert(path.clone());
                    l.app.task(Task::Directory {
                        project,
                        path,
                        offset: 0,
                    });
                }
                "open" => {
                    live_mut(s).app.open_file(project, path);
                    panels::show(s, panels::Screen::Files);
                }
                "relative" => tx.copy(path),
                "full" => tx.copy(std::path::Path::new(&project).join(path).to_string_lossy()),
                "reveal" => live_mut(s).app.task(Task::Reveal { project, path }),
                "attach" if live(s).app.selected == owner => {
                    if let Some(index) =
                        owner.filter(|&i| live(s).app.sessions[i].scope.project == project)
                    {
                        live_mut(s).app.attach_file(index, path);
                    }
                }
                "close" => {
                    if let Some(c) = live_mut(s).app.services.checkouts.get_mut(&project) {
                        c.tabs.retain(|p| p != &path);
                        if c.selected_file.as_ref() == Some(&path) {
                            c.selected_file = c.tabs.last().cloned();
                        }
                    }
                }
                _ => {}
            });
        })
}

pub fn link(s: &mut Workspace, index: usize, url: &str, at: PointerPress, _: &mut Tx) {
    let url = url.to_owned();
    let origin = live(s)
        .focus_key
        .clone()
        .unwrap_or_else(|| format!("live-draft-{index}"));
    let menu = Menu::new()
        .item(Item::new("open", "Open link").icon(&ui::EXTERNAL))
        .item(Item::new("copy", "Copy link address").icon(&glyphs::COPY));
    show(s, at, origin, menu, move |s, pick, tx| match pick {
        "open" => panels::open_link(s, index, &url, tx),
        "copy" => tx.copy(url),
        _ => {}
    });
}

pub fn pull_request(el: El, project: String, url: String) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let project = project.clone();
        let url = url.clone();
        let menu = Menu::new()
            .item(Item::new("open", "Open pull request").icon(&ui::BRANCH))
            .item(Item::new("browser", "Open in browser").icon(&ui::EXTERNAL))
            .separator()
            .item(Item::new("copy", "Copy pull request URL").icon(&glyphs::COPY));
        show(
            s,
            at,
            crate::code_view::source_key(&url, "pull-request"),
            menu,
            move |s, pick, tx| match pick {
                "open" => panels::open_review(s, project, url),
                "browser" => {
                    let _ = tx.open_url(&url);
                }
                "copy" => tx.copy(url),
                _ => {}
            },
        );
    })
}

pub fn session(el: El, index: usize) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let session = &live(s).app.sessions[index];
        let archived = session.archived;
        let title = session.title.clone();
        let id = session.remote_id.clone();
        let mut menu = Menu::new()
            .item(Item::new("copy", "Copy session title").icon(&glyphs::COPY))
            .item(Item::new("id", "Copy session ID").enabled(id.is_some()))
            .item(
                Item::new("refresh", "Refresh session")
                    .icon(&ui::REFRESH)
                    .enabled(
                        session.remote_id.is_some() && !session.loading && !session.submitting,
                    ),
            )
            .separator()
            .item(
                Item::new(
                    "archive",
                    if archived {
                        "Restore session"
                    } else {
                        "Archive session"
                    },
                )
                .icon(&ui::ARCHIVE)
                .enabled(!crate::live::archive_disabled(live(s), index)),
            );
        if archived {
            menu = menu.separator().item(
                Item::new("delete", "Delete session permanently")
                    .enabled(!crate::live::archive_disabled(live(s), index)),
            );
        }
        // Restoring the row itself would follow the list's selected session.
        // Its action button keeps keyboard focus on the right-clicked owner.
        show(
            s,
            at,
            format!("session-actions-{index}"),
            menu,
            move |s, pick, tx| match pick {
                "copy" => tx.copy(title.clone()),
                "id" => tx.copy(id.unwrap_or_default()),
                "refresh" => live_mut(s).app.refresh_session(index),
                "archive" if !crate::live::archive_disabled(live(s), index) => {
                    live_mut(s).archive(index, !archived)
                }
                "delete" if archived && !crate::live::archive_disabled(live(s), index) => {
                    live_mut(s).request_delete_session(index)
                }
                _ => {}
            },
        );
    })
}

pub fn workspace(s: &mut Workspace, at: PointerPress, tx: &mut Tx) {
    if s.live.is_none() {
        return;
    }
    let selected = tx
        .text_selection()
        .map(|selection| selection.text().to_owned());
    let origin = live(s)
        .focus_key
        .clone()
        .unwrap_or_else(|| "new-session".into());
    let mut menu = Menu::new();
    if selected.is_some() {
        menu = menu
            .item(Item::new("copy", "Copy selection").icon(&glyphs::COPY))
            .separator();
    }
    menu = menu
        .item(
            Item::new("new", "New session")
                .icon(&crate::PLUS)
                .shortcut("⌘N"),
        )
        .item(
            Item::new("open", "Open project…")
                .icon(&ui::FOLDER)
                .shortcut("⌘O"),
        )
        .separator()
        .item(
            Item::new("settings", "Settings…")
                .icon(&ui::SETTINGS)
                .shortcut("⌘,"),
        );
    show(s, at, origin, menu, move |s, pick, tx| match pick {
        "copy" => tx.copy(selected.unwrap_or_default()),
        "new" => s.new_session(),
        "open" => crate::live::open_project(s, tx),
        "settings" => panels::show(s, panels::Screen::Settings),
        _ => {}
    });
}

/// Wire the model and its menu together so editing from a menu uses the same
/// undo history, change notifications and paste route as typing in the field.
pub trait EditMenu {
    fn edit_menu(
        self,
        key: impl Into<String>,
        edit: impl Fn(&mut Workspace) -> Option<&mut TextEdit> + 'static,
    ) -> El;
    fn edit_menu_changed(
        self,
        key: impl Into<String>,
        edit: impl Fn(&mut Workspace) -> Option<&mut TextEdit> + 'static,
        changed: impl Fn(&mut Workspace, &mut Tx) + 'static,
    ) -> El;
}

impl EditMenu for El {
    fn edit_menu(
        self,
        key: impl Into<String>,
        edit: impl Fn(&mut Workspace) -> Option<&mut TextEdit> + 'static,
    ) -> El {
        self.edit_menu_changed(key, edit, |_, _| {})
    }

    fn edit_menu_changed(
        self,
        key: impl Into<String>,
        edit: impl Fn(&mut Workspace) -> Option<&mut TextEdit> + 'static,
        changed: impl Fn(&mut Workspace, &mut Tx) + 'static,
    ) -> El {
        let key = key.into();
        let edit = Rc::new(edit);
        let model = edit.clone();
        let changed = Rc::new(changed);
        let change = changed.clone();
        self.key(&key)
            .on_edit(move |s| model(s).expect("mounted editor"))
            .on_input_change(move |s, tx| change(s, tx))
            .on_secondary_press(move |s: &mut Workspace, at, _| {
                if s.live.is_none() {
                    return;
                }
                let Some(editor) = edit(s) else {
                    return;
                };
                let snapshot = editor.clone();
                let mut undo = editor.clone();
                undo.undo();
                let mut redo = editor.clone();
                redo.redo();
                let selected = !editor.selected().is_empty() && !editor.is_secure();
                let menu = Menu::new()
                    .item(
                        Item::new("undo", "Undo")
                            .shortcut("⌘Z")
                            .enabled(undo.text != editor.text),
                    )
                    .item(
                        Item::new("redo", "Redo")
                            .shortcut("⇧⌘Z")
                            .enabled(redo.text != editor.text),
                    )
                    .separator()
                    .item(Item::new("cut", "Cut").shortcut("⌘X").enabled(selected))
                    .item(Item::new("copy", "Copy").shortcut("⌘C").enabled(selected))
                    .item(Item::new("paste", "Paste").shortcut("⌘V"))
                    .separator()
                    .item(
                        Item::new("all", "Select all")
                            .shortcut("⌘A")
                            .enabled(!editor.text.is_empty()),
                    );
                let edit = edit.clone();
                let changed = changed.clone();
                let origin = key.clone();
                show(s, at, key.clone(), menu, move |s, pick, tx| {
                    let Some(editor) = edit(s) else {
                        return;
                    };
                    if editor.text != snapshot.text || editor.range() != snapshot.range() {
                        return;
                    }
                    editor.focused = true;
                    match pick {
                        "undo" => editor.undo(),
                        "redo" => editor.redo(),
                        "cut" => {
                            tx.copy(editor.selected());
                            editor.insert("");
                        }
                        "copy" => tx.copy(editor.selected()),
                        "paste" => {
                            let l = live_mut(s);
                            l.context.paste = Some((origin, Scope::of(l), false));
                            tx.wake_after(std::time::Duration::ZERO);
                            return;
                        }
                        "all" => editor.set_selection(0, editor.text.len()),
                        _ => {}
                    }
                    if editor.text != snapshot.text {
                        changed(s, tx);
                    }
                });
            })
    }
}

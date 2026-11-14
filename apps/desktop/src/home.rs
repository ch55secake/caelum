//! Home owns its native editor and UI state independently of every session.
mod attachments;
mod controls;
mod scenes;
mod view;

pub(crate) use scenes::scene;
pub use view::view;

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels::Screen,
};
use rune::{Image, Key, KeyEvent, ScrollPos, TextEdit, Tx};
use std::{cell::Cell, rc::Rc};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Menu {
    #[default]
    Closed,
    Projects,
    Provider,
    Effort,
}

pub(crate) struct Background {
    pub dark: Image,
    pub light: Image,
}

pub struct State {
    pub editor: TextEdit,
    pub(crate) background: Option<Background>,
    position: Rc<Cell<ScrollPos>>,
    mounted: bool,
    menu: Menu,
    catalogue: Option<caelum_core::application::Scope>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            editor: TextEdit::new(""),
            background: None,
            position: Default::default(),
            mounted: false,
            menu: Menu::Closed,
            catalogue: None,
        }
    }
}

pub fn is_home(s: &Workspace) -> bool {
    s.live
        .as_ref()
        .is_some_and(|l| l.panels.screen == Screen::Home)
}

pub fn workspace(project: Option<String>) -> Workspace {
    let home = project.is_none();
    let mut s = Workspace::live(project);
    if home {
        open(&mut s);
    }
    s
}

pub fn open(s: &mut Workspace) {
    let l = live_mut(s);
    l.panels.remember(&mut l.app);
    l.app.leave_conversation();
    l.panels.screen = Screen::Home;
    l.panels.overlay = None;
    l.session_menu = None;
    l.models = Default::default();
    l.home.menu = Menu::Closed;
    l.completion = Default::default();
    for draft in &mut l.drafts {
        draft.focused = false;
    }
    l.home.editor.focused = true;
    s.inspector.set_open(false);
}

pub fn changed(s: &mut Workspace, _: &mut Tx) {
    let l = live_mut(s);
    l.app.home.draft.text.clone_from(&l.home.editor.text);
    l.app.home.error = None;
}

pub fn send(s: &mut Workspace, tx: &mut Tx) {
    let l = live_mut(s);
    l.app.home.draft.text.clone_from(&l.home.editor.text);
    if !l.app.home_project_available() {
        l.home.menu = Menu::Projects;
        return;
    }
    if let Some(index) = l.app.send_home() {
        l.home.editor.clear();
        l.home.editor.focused = false;
        l.home.position.set(ScrollPos::default());
        l.home.menu = Menu::Closed;
        l.sync_editors();
        l.panels.screen = Screen::Conversation;
        l.app.conversation_hidden = false;
        l.focus();
        tx.scroll_to_bottom_instant(format!("live-transcript-{index}"));
    }
}

pub fn sync(l: &mut Live, tx: &mut Tx) -> bool {
    let shown = l.panels.screen == Screen::Home;
    let mut changed = false;
    if shown && l.home.background.is_none() {
        // Two 2000×1122 assets (under 24 MiB total including mips), decoded on
        // Rune's image worker after launch and reused across theme changes.
        let background = Background {
            dark: l.media.home_background(true),
            light: l.media.home_background(false),
        };
        if l.fixture {
            let _ = background.dark.wait();
            let _ = background.light.wait();
        }
        l.home.background = Some(background);
        changed = true;
    }
    if shown
        && l.app.home.draft.project.is_none()
        && !l.app.home.opening()
        && let Some(project) = l.app.active_project.clone()
    {
        l.app.home_select_project(project);
        changed = true;
    }
    let scope = shown.then(|| l.app.home_scope()).flatten();
    if scope != l.home.catalogue {
        if let Some(scope) = &scope
            && !l.app.choices.contains_key(scope)
        {
            l.app.refresh_choices_for(scope.clone());
        }
        l.home.catalogue = scope;
    }
    if shown != l.home.mounted {
        l.home.mounted = shown;
        if shown {
            tx.scroll_to_y("home-editor", l.home.position.get().offset[1]);
        } else {
            l.home.editor.focused = false;
            l.home.menu = Menu::Closed;
        }
        changed = true;
    }
    changed
}

pub fn on_key(s: &mut Workspace, ev: &KeyEvent, _: &mut Tx) -> bool {
    if !is_home(s) {
        return false;
    }
    if ev.key == Key::Escape && live(s).home.menu != Menu::Closed {
        live_mut(s).home.menu = Menu::Closed;
        live_mut(s).focus();
        return true;
    }
    false
}

fn close_menu(s: &mut Workspace) {
    live_mut(s).home.menu = Menu::Closed;
}

fn toggle_menu(s: &mut Workspace, menu: Menu) {
    let l = live_mut(s);
    l.home.menu = if l.home.menu == menu {
        Menu::Closed
    } else {
        menu
    };
}

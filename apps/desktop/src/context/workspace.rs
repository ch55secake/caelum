use super::{Item, Menu, is_open, show};
use crate::{Workspace, live::live, panels, quiet_button, ui};
use rune::{El, PointerPress, Tx};
use rune_kit::{Kit, glyphs};

const TRIGGER: &str = "workspace-trigger";

pub(crate) fn button(s: &Workspace, k: &Kit) -> El {
    let open = is_open(s)
        && live(s)
            .context
            .opened
            .as_ref()
            .is_some_and(|m| m.point.is_none());
    quiet_button(&ui::MORE, "Workspace menu")
        .expanded(open)
        .on_press(|s: &mut Workspace| open_menu(s, None, TRIGGER.into(), None))
        .build(k)
        .key(TRIGGER)
}

pub(crate) fn workspace(s: &mut Workspace, at: PointerPress, tx: &mut Tx) {
    let Some(l) = &s.live else { return };
    let origin = l.focus_key.clone().unwrap_or_else(|| TRIGGER.into());
    let selected = tx
        .text_selection()
        .map(|selection| selection.text().to_owned());
    open_menu(s, Some(at), origin, selected);
}

fn open_menu(
    s: &mut Workspace,
    at: Option<PointerPress>,
    origin: String,
    selected: Option<String>,
) {
    let checkout = live(s).app.active_project.is_some();
    let remote = live(s).panels.screen == panels::Screen::Remote;
    let mut menu = Menu::new();
    if selected.is_some() {
        menu = menu
            .item(Item::new("copy", "Copy selection").icon(&glyphs::COPY))
            .separator();
    }
    menu = menu
        .item(
            Item::new("files", "Find a file…")
                .icon(&glyphs::SEARCH)
                .shortcut("⌘P")
                .enabled(checkout),
        )
        .item(
            Item::new(
                "changes",
                if remote || live(s).native.is_some() {
                    "View remote changes"
                } else {
                    "View local changes"
                },
            )
            .icon(&ui::BRANCH)
            .enabled(checkout || remote),
        )
        .separator()
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
        "files" => panels::find_files(s, false),
        "changes" if remote => crate::remote::open_changes(s),
        "changes" => panels::show(s, panels::Screen::Changes),
        "new" => s.new_session(),
        "open" => crate::live::open_project(s, tx),
        "settings" => panels::show(s, panels::Screen::Settings),
        _ => {}
    });
}

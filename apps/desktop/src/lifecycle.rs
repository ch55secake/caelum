use crate::{Workspace, live, panels};
use rune::{App, MenuItem, Shortcut, Tx};

pub fn configure(app: App<Workspace>) -> App<Workspace> {
    app.keep_running()
        .standard_menus()
        .on_focus(|s, focused, _| {
            if s.live.is_some() {
                let live = live::live_mut(s);
                live.app.window_background = !focused;
                if focused && !live.fixture && !live.evaluation {
                    live.app.refresh_files();
                }
            }
        })
        .on_open_url(open_url)
        .on_close(|s, tx| {
            if let Some(live) = &mut s.live {
                live.poll(tx);
            }
        })
        .menu_item(
            MenuItem::new("Settings…", |s: &mut Workspace, _| {
                if s.live.is_some() {
                    panels::show(s, panels::Screen::Settings);
                }
                if let Some(handle) = &s.handle {
                    handle.open();
                }
            })
            .key_equivalent("cmd+comma".parse::<Shortcut>().expect("shortcut"))
            .keys_first(),
        )
        .menu_item(
            MenuItem::new("Open Project…", |s: &mut Workspace, tx| {
                if let Some(handle) = &s.handle {
                    handle.open();
                }
                if s.live.is_some() {
                    live::open_project(s, tx);
                }
            })
            .menu("File")
            .key_equivalent("cmd+o".parse::<Shortcut>().expect("shortcut"))
            .keys_first(),
        )
        .menu_item(
            MenuItem::new("New Session", |s: &mut Workspace, _| {
                s.new_session();
                if let Some(handle) = &s.handle {
                    handle.open();
                }
            })
            .menu("File")
            .key_equivalent("cmd+n".parse::<Shortcut>().expect("shortcut"))
            .keys_first(),
        )
        .menu_item(
            MenuItem::new("Caelum Releases", |_: &mut Workspace, tx| {
                let _ = tx.open_url(caelum_core::updates::RELEASES_URL);
            })
            .menu("Help"),
        )
}

pub fn open_url(s: &mut Workspace, url: &str, _: &mut Tx) {
    let Some(live) = &mut s.live else {
        return;
    };
    if live.evaluation || live.fixture {
        return;
    }
    match caelum_core::desktop::project_from_url(url) {
        Ok(path) => {
            live.panels.screen = panels::Screen::Conversation;
            live.panels.overlay = None;
            live.app.open_project(path.to_string_lossy().into_owned());
        }
        Err(error) => live.app.error = Some(error),
    }
    if let Some(handle) = &s.handle {
        handle.open();
    }
}

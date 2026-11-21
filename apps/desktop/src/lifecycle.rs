#[cfg(target_os = "linux")]
mod install;
mod launch;
#[cfg(target_os = "linux")]
mod linux;

pub(crate) use launch::run;

use crate::{Workspace, live, panels};
use rune::{App, Chrome, KeyEvent, MenuItem, Shortcut, Tx};

pub fn configure(app: App<Workspace>) -> App<Workspace> {
    #[cfg(target_os = "macos")]
    let app = app.keep_running();
    app.standard_menus()
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
                if live.native.is_some() {
                    tx.quit();
                }
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

fn chrome() -> Chrome {
    if cfg!(target_os = "macos") {
        Chrome {
            full_size_content: true,
            traffic_lights: Some((16.0, 17.0)),
            drag_regions: true,
        }
    } else {
        Chrome::default()
    }
}

pub(super) fn on_key(s: &mut Workspace, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    if ev.is_command('q') {
        tx.quit();
    } else if ev.is_command('w') {
        tx.close_window();
    } else if ev.is_command(',') && s.live.is_some() {
        panels::show(s, panels::Screen::Settings);
    } else {
        return false;
    }
    true
}

pub fn open_url(s: &mut Workspace, url: &str, _: &mut Tx) {
    // The release smoke test uses this marker to distinguish an OS URL event
    // that never reached the app from one whose project-open request stalled.
    if let Some(path) = std::env::var_os("CAELUM_SMOKE_CALLBACK_TRACE") {
        use std::{fs::OpenOptions, io::Write};
        if let Ok(mut trace) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(trace, "received");
        }
    }
    let Some(state) = &s.live else {
        return;
    };
    if state.evaluation || state.fixture || state.native.is_some() {
        return;
    }
    match caelum_core::desktop::project_from_url(url) {
        Ok(path) => activate(s, Some(path.to_string_lossy().into_owned())),
        Err(error) => live::live_mut(s).app.error = Some(error),
    }
}

fn activate(s: &mut Workspace, project: Option<String>) {
    // Activation can arrive while idle. Schedule the queued project-open effect.
    if let Some(project) = project
        && s.live.as_ref().is_some_and(|l| l.native.is_none())
    {
        let live = live::live_mut(s);
        live.panels.screen = panels::Screen::Conversation;
        live.panels.overlay = None;
        live.app.open_project(project);
    }
    if let Some(handle) = &s.handle {
        handle.open();
    }
}

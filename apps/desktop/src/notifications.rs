//! Native delivery and click routing; notification eligibility remains core-owned.
#[cfg(target_os = "macos")]
mod macos;

use crate::{
    Workspace,
    live::{Live, live_mut},
    panels::Screen,
};
use caelum_core::application::{Notification, SessionKey};
use rune::AppHandle;

enum Activation {
    App,
    Session(SessionKey),
}

#[derive(Default)]
pub struct State {
    #[cfg(target_os = "macos")]
    client: Option<macos::Client>,
    activation: Option<Activation>,
}

impl State {
    pub fn connect(handle: AppHandle<Workspace>, enabled: bool) -> Self {
        let mut state = Self::default();
        if enabled {
            #[cfg(target_os = "macos")]
            match macos::Client::new(handle) {
                Ok(client) => state.client = Some(client),
                Err(error) => eprintln!("Notification: {error}"),
            }
            #[cfg(not(target_os = "macos"))]
            let _ = handle;
        }
        state
    }

    pub fn send(&self, request: u64, notification: Notification) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let client = self
                .client
                .as_ref()
                .ok_or("Native notifications require an app bundle; run make dev")?;
            client.send(request, notification);
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (request, notification);
            Err("Native notifications are unavailable on this platform".into())
        }
    }
}

fn clicked(s: &mut Workspace, target: Option<SessionKey>) {
    if s.live.is_some() {
        live_mut(s).notifications.activation =
            Some(target.map_or(Activation::App, Activation::Session));
    }
    if let Some(handle) = &s.handle {
        // This also recreates a closed window and restores a minimized one.
        handle.open();
    }
}

pub fn dispatch(live: &mut Live) -> bool {
    let Some(activation) = live.notifications.activation.take() else {
        return false;
    };
    live.panels.remember(&mut live.app);
    if let Activation::Session(target) = activation {
        live.app.activate_notification(target);
    }
    live.panels.screen = Screen::Conversation;
    live.panels.overlay = None;
    live.panels.archived = false;
    live.models = Default::default();
    live.session_menu = None;
    live.commands.side_owner = None;
    live.sync_editors();
    live.focus();
    true
}

#[cfg(test)]
mod tests;

//! Native updater presentation and the final, post-shutdown restart handoff.
mod scenes;

pub(crate) use scenes::scene;

use crate::{
    Workspace,
    live::{Live, live, live_mut},
    panels::Screen,
    ui::{self, Button},
};
use caelum_core::{
    application::Task,
    updates::{Installed, Job, Phase, Progress, Retry},
};
use rune::{El, Tx, col, row, text};
use rune_kit::{ButtonVariant, Kit};
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

pub(crate) struct Restart {
    installed: Arc<Installed>,
    project: Option<String>,
}
pub(crate) type RestartSlot = Arc<Mutex<Option<Restart>>>;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Startup {
    #[default]
    Frame,
    Store,
    Scheduled,
}

#[derive(Default)]
pub(crate) struct Lifecycle {
    startup: Cell<Startup>,
    pub restart: RestartSlot,
}

impl Lifecycle {
    pub fn abort_restart(&self) {
        self.restart
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

pub(crate) fn drawn(rt: &rune::Runtime<Workspace>) {
    if let Some(live) = &rt.state.live
        && !live.fixture
        && !live.evaluation
        && live.updates.startup.get() == Startup::Frame
        && let Some(handle) = &rt.state.handle
    {
        live.updates.startup.set(Startup::Store);
        handle.update(|_, _| {});
    }
}

pub(crate) fn poll(live: &mut Live, tx: &mut Tx) -> bool {
    if live.updates.startup.get() == Startup::Store
        && live.app.restored()
        && live.app.persistence_error().is_none()
    {
        live.updates.startup.set(Startup::Scheduled);
        if matches!(live.app.services.updater.phase, Phase::Idle) {
            let job = live.app.services.updater.startup();
            live.app.task(Task::Update(job));

            tx.wake_after(std::time::Duration::ZERO);
        }
    }
    if let Phase::Restarting(installed) = &live.app.services.updater.phase {
        if let Some(error) = live.app.persistence_error() {
            live.app
                .services
                .updater
                .restart_failed(format!("Save your workspace before restarting: {error}"));
            return true;
        }
        if live.app.saved_for_restart() {
            *live
                .updates
                .restart
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(Restart {
                installed: installed.clone(),
                project: (live.panels.screen != Screen::Home)
                    .then(|| live.app.active_project.clone())
                    .flatten(),
            });
            tx.quit();
        }
    }
    false
}

pub(crate) fn relaunch(slot: RestartSlot) {
    if let Some(restart) = slot.lock().unwrap_or_else(|e| e.into_inner()).take()
        && let Err(error) = restart.installed.relaunch(restart.project.as_deref())
    {
        eprintln!("{error}");
    }
}

fn dispatch(s: &mut Workspace, job: Option<Job>) {
    if let Some(job) = job {
        live_mut(s).app.task(Task::Update(job));
    }
}

fn cancel(s: &mut Workspace) {
    let job = live_mut(s).app.services.updater.cancel();
    dispatch(s, job);
}

pub(crate) fn view(s: &Workspace, k: &Kit) -> El {
    let services = &live(s).app.services;
    let phase = &services.updater.phase;
    if matches!(phase, Phase::Idle | Phase::Updated(_)) {
        return discovery(s, k);
    }
    let mut actions = row().w_full().gap(8.0).center_y();
    let (title, detail, busy) = match phase {
        Phase::Downloading { release, progress } => {
            actions = actions.child(
                Button::new("Cancel")
                    .variant(ButtonVariant::Outline)
                    .on_press(cancel)
                    .build(k)
                    .key("cancel-update"),
            );
            match progress {
                Progress::Downloading { bytes, total } => (
                    "Downloading update",
                    format!(
                        "Version {} · {:.1} MiB{}",
                        release.latest_version,
                        *bytes as f64 / 1_048_576.0,
                        total
                            .map(|n| format!(" of {:.1} MiB", n as f64 / 1_048_576.0))
                            .unwrap_or_default()
                    ),
                    true,
                ),
                Progress::Verifying => (
                    "Verifying update",
                    "Checking the download against the release checksums…".into(),
                    true,
                ),
                Progress::Preparing => (
                    "Preparing update",
                    "Checking and staging the application…".into(),
                    true,
                ),
            }
        }
        Phase::Cancelling => (
            "Cancelling update",
            "Removing the staged download…".into(),
            true,
        ),
        Phase::Ready(prepared) => {
            actions = actions
                .child(
                    Button::new("Install update")
                        .variant(ButtonVariant::Contrast)
                        .on_press(|s: &mut Workspace| {
                            let job = live_mut(s).app.services.updater.install();
                            dispatch(s, job);
                        })
                        .build(k)
                        .key("install-update"),
                )
                .child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(cancel)
                        .build(k)
                        .key("cancel-update"),
                );
            (
                "Ready to install",
                format!(
                    "Version {} has been downloaded and verified.",
                    prepared.release.latest_version
                ),
                false,
            )
        }
        Phase::Installing(_) => (
            "Installing update",
            "Replacing the application and retaining the previous version…".into(),
            true,
        ),
        Phase::Installed(installed) => {
            actions = actions.child(
                Button::new("Restart Caelum")
                    .variant(ButtonVariant::Contrast)
                    .on_press(|s: &mut Workspace| live_mut(s).app.services.updater.restart())
                    .build(k)
                    .key("restart-update"),
            );
            (
                "Update installed",
                format!(
                    "Restart to use version {}. Your workspace will be saved; owned agent processes stop during restart.",
                    installed.version()
                ),
                false,
            )
        }
        Phase::Restarting(_) => (
            "Saving workspace",
            "Waiting for your latest drafts and queues before restarting…".into(),
            true,
        ),
        Phase::Failed { message, retry } => {
            actions = actions.child(
                Button::new("Try again")
                    .variant(ButtonVariant::Outline)
                    .on_press(|s: &mut Workspace| {
                        if matches!(
                            live(s).app.services.updater.phase,
                            Phase::Failed {
                                retry: Retry::Restart(_),
                                ..
                            }
                        ) {
                            live_mut(s).app.retry_save();
                        }
                        let job = live_mut(s).app.services.updater.retry();
                        dispatch(s, job);
                    })
                    .build(k)
                    .key("retry-update"),
            );
            if matches!(retry, Retry::Download(_) | Retry::Install(_)) {
                actions = actions.child(
                    Button::new("Cancel")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press(cancel)
                        .build(k)
                        .key("cancel-update"),
                );
            }
            if let Some(update) = &services.update {
                let url = update
                    .download_url
                    .clone()
                    .unwrap_or_else(|| update.release_url.clone());
                actions = actions.child(
                    Button::new("Download in browser")
                        .variant(ButtonVariant::GhostMuted)
                        .on_press_tx(move |_: &mut Workspace, tx| {
                            let _ = tx.open_url(&url);
                        })
                        .build(k)
                        .key("manual-update"),
                );
            }
            ("Couldn’t update Caelum", message.clone(), false)
        }
        Phase::Idle | Phase::Updated(_) => unreachable!(),
    };
    let mut panel = col()
        .w_full()
        .gap(10.0)
        .padding(14.0)
        .radius(8.0)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_muted)
        .child(
            col()
                .w_full()
                .gap(4.0)
                .role("status")
                .label(format!("{title}. {detail}"))
                .child(if busy {
                    ui::busy_text(title, k.ty.ui_medium, k)
                } else {
                    text(title).font(k.ty.ui_medium).color(k.c.text_base)
                })
                .child(ui::caption(detail, k).selectable(true)),
        );
    if let Phase::Downloading {
        progress: Progress::Downloading { bytes, total },
        ..
    } = phase
    {
        let fraction = total
            .filter(|n| *n > 0)
            .map(|n| (*bytes as f32 / n as f32).clamp(0.0, 1.0));
        panel = panel.child(
            rune_kit::Progress::new(fraction)
                .label("Update download progress")
                .build(k),
        );
    }
    panel.child(actions).key("about-updates")
}

fn discovery(s: &Workspace, k: &Kit) -> El {
    use caelum_protocol::UpdateStatus;
    let services = &live(s).app.services;
    let busy = services.busy(&Task::Updates);
    let error = services.error(&Task::Updates);
    let update = services.update.as_ref();
    let (title, detail) = if busy {
        (
            "Checking for updates",
            "Looking for the latest release…".into(),
        )
    } else if let Some(error) = error {
        ("Couldn’t check for updates", error.to_owned())
    } else if let Some(update) = update {
        match update.status {
            UpdateStatus::Available => (
                "Update available",
                format!("Version {} is ready to download.", update.latest_version),
            ),
            UpdateStatus::UpToDate => (
                "You’re up to date",
                "You’re running the latest release.".into(),
            ),
            UpdateStatus::Ahead => (
                "Ahead of the latest release",
                format!("Latest stable version: {}", update.latest_version),
            ),
        }
    } else if let Phase::Updated(version) = &services.updater.phase {
        (
            "Update installed",
            format!("Successfully updated to version {version}."),
        )
    } else {
        (
            "Software updates",
            "Keep Caelum up to date with the latest release.".into(),
        )
    };
    let marker = if !busy && error.is_some() {
        rune::icon(&rune_kit::glyphs::CIRCLE_X, 16.0).color(k.c.danger.fg)
    } else if !busy && update.is_some_and(|u| u.status == UpdateStatus::UpToDate) {
        rune::icon(&rune_kit::glyphs::CHECK, 16.0).color(k.c.success.fg)
    } else {
        rune::icon(&ui::REFRESH, 16.0).color(k.c.icon_muted)
    };
    let mut actions = row().gap(6.0).center_y();
    if let Some(update) =
        update.filter(|u| !busy && error.is_none() && u.status == UpdateStatus::Available)
    {
        let update = update.clone();
        if update.download_url.is_some() {
            actions = actions.child(
                Button::new("Download update")
                    .variant(ButtonVariant::Contrast)
                    .on_press(move |s: &mut Workspace| {
                        let job = live_mut(s).app.services.updater.download(&update);
                        dispatch(s, job);
                    })
                    .build(k)
                    .key("download-update"),
            );
        } else {
            actions = actions.child(
                Button::new("View release")
                    .variant(ButtonVariant::Contrast)
                    .on_press_tx(move |_: &mut Workspace, tx| {
                        let _ = tx.open_url(&update.release_url);
                    })
                    .build(k)
                    .key("view-release"),
            );
        }
        actions = actions.child(
            crate::quiet_button(&ui::REFRESH, "Check for updates")
                .on_press(|s: &mut Workspace| live_mut(s).app.task(Task::Updates))
                .build(k)
                .key("check-updates"),
        );
    } else {
        actions = actions.child(
            Button::new(if busy {
                "Checking…"
            } else if error.is_some() {
                "Try again"
            } else if update.is_some() {
                "Check again"
            } else {
                "Check for updates"
            })
            .variant(ButtonVariant::Outline)
            .disabled(busy)
            .on_press(|s: &mut Workspace| live_mut(s).app.task(Task::Updates))
            .build(k)
            .key("check-updates"),
        );
    }
    row()
        .w_full()
        .min_h(88.0)
        .padding(14.0)
        .gap(12.0)
        .center_y()
        .radius(8.0)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_muted)
        .child(marker)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(4.0)
                .role("status")
                .label(format!("{title}. {detail}"))
                .child(if busy {
                    ui::busy_text(title, k.ty.ui_medium, k)
                } else {
                    text(title).font(k.ty.ui_medium).color(k.c.text_base).wrap()
                })
                .child(ui::caption(detail, k).selectable(true)),
        )
        .child(actions)
        .key("about-updates")
}

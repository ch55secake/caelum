use rune::{El, col};
use rune_kit::{Dialog, Kit};

use crate::Workspace;
use crate::live::{live, live_mut};

use super::Screen;
use super::launcher::launcher;

pub fn overlays(s: &Workspace, k: &Kit, layers: &mut Vec<El>) {
    let Some(screen) = live(s).panels.overlay else {
        return;
    };
    let mut menus = Vec::new();
    let (title, description, body) = match screen {
        Screen::Settings => ("Settings", "", crate::settings::view(s, k, &mut menus)),
        Screen::Worktrees => (
            "Caelum worktrees",
            "Created in Caelum for this project.",
            crate::checkouts::manager(s, k),
        ),
        Screen::CreateWorktree => (
            "Create worktree",
            "A fresh checkout for an independent session.",
            crate::worktree::view(s, k, &mut menus),
        ),
        Screen::Shells => (
            "Background commands",
            "Live output from this session.",
            crate::shells::panel(s, k),
        ),
        Screen::Timeline => (
            "Timeline",
            "Find and reveal an earlier prompt.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Sessions => (
            "Sessions",
            "Browse earlier conversations from this provider.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Usage => (
            "OpenCode Go usage",
            "Subscription allowances.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Stats => (
            "Statistics",
            "Tokens, cost and activity across recorded history.",
            crate::commands::view(s, k, screen, &mut menus),
        ),
        Screen::Image => (
            "Image preview",
            "Fit, zoom and scroll the image.",
            crate::media::view(s, k),
        ),
        Screen::Attachment => {
            let (title, description) = crate::composer::preview_header(s);
            (title, description, crate::composer::preview(s, k))
        }
        Screen::Launcher => (
            "New session",
            "Choose a project and provider.",
            launcher(s, k),
        ),
        _ => return,
    };
    let (width, height) = match screen {
        Screen::Settings => (740.0, crate::settings::height(s)),
        Screen::Launcher => (560.0, 380.0),
        Screen::Worktrees => (560.0, crate::checkouts::manager_height(s)),
        Screen::CreateWorktree => (500.0, 330.0),
        Screen::Timeline => (580.0, 344.0),
        Screen::Sessions => (580.0, 420.0),
        Screen::Stats => (720.0, 620.0),
        Screen::Image => (760.0, 480.0),
        Screen::Attachment => (760.0, crate::composer::preview_height(s)),
        _ => (660.0, 420.0),
    };
    let available =
        rune::memo::window_layout("shell").map_or(height, |r| (r[3] - 170.0).max(220.0));
    let dialog = Dialog::new("workspace", title);
    let dialog = if description.is_empty() {
        dialog
    } else {
        dialog.description(description)
    };
    dialog
        .open(true)
        .click_outside(false)
        .width(width)
        .fit()
        .on_close(|s: &mut Workspace| {
            live_mut(s).panels.overlay = None;
            live_mut(s).focus();
        })
        .child(
            col()
                .w_full()
                .h(height.min(available))
                .min_h(0.0)
                .px(if screen == Screen::Attachment {
                    16.0
                } else {
                    24.0
                })
                .pb(if screen == Screen::Attachment {
                    16.0
                } else {
                    24.0
                })
                .child(body),
        )
        .build(k, layers);
    layers.extend(menus);
}

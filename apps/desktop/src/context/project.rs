use super::{Item, Menu, show};
use crate::{
    Workspace,
    live::{live, live_mut},
    ui,
};
use caelum_core::application::{ProjectMove, Task};
use rune::El;
use rune_kit::glyphs;

pub(crate) fn remote_checkout(el: El, id: String, origin: String) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let id = id.clone();
        let menu = Menu::new()
            .item(Item::new("open", "Open VM conversation").icon(&ui::SERVER))
            .item(
                Item::new("native", "Open native workspace…")
                    .icon(&ui::SERVER)
                    .enabled(crate::remote::native::target(live(s), &id).is_some()),
            )
            .item(
                Item::new("new", "New session on this VM")
                    .icon(&crate::PLUS)
                    .enabled(live(s).app.can_start_vm_session(&id)),
            )
            .separator()
            .item(
                Item::new(
                    "remove",
                    if live(s)
                        .app
                        .remote
                        .saved
                        .checkouts
                        .get(&id)
                        .and_then(|c| c.machine())
                        .is_some()
                    {
                        "Delete VM on Boat…"
                    } else {
                        "Remove from project…"
                    },
                )
                .icon(&glyphs::CLOSE)
                .enabled(live(s).app.can_remove_remote(&id)),
            );
        show(s, at, origin.clone(), menu, move |s, pick, _| match pick {
            "open" => crate::remote::select(s, &id),
            "native" => crate::remote::native::launch(s, &id),
            "new" => crate::remote::new_session(s, &id),
            "remove" => crate::remote::request_removal(s, &id),
            _ => {}
        });
    })
}

pub(crate) fn project(el: El, project: String) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, _| {
        let project = project.clone();
        let origin = crate::code_view::source_key(&project, "project");
        let app = &live(s).app;
        let menu = Menu::new()
            .item(Item::new("new", "New session").icon(&crate::PLUS))
            .item(
                Item::new("worktree", "Create worktree…")
                    .icon(&ui::BRANCH)
                    .enabled(live(s).native.is_none()),
            )
            .item(
                Item::new("remote", "Open on remote machine…")
                    .icon(&ui::SERVER)
                    .enabled(live(s).native.is_none()),
            )
            .separator()
            .item(
                Item::new("up", "Move project up")
                    .icon(&glyphs::CHEVRON_UP)
                    .enabled(app.can_move_project(&project, ProjectMove::Up)),
            )
            .item(
                Item::new("down", "Move project down")
                    .icon(&glyphs::CHEVRON_DOWN)
                    .enabled(app.can_move_project(&project, ProjectMove::Down)),
            )
            .separator()
            .item(Item::new("copy", "Copy project path").icon(&glyphs::COPY))
            .item(
                Item::new("reveal", "Reveal in Finder")
                    .icon(&ui::FOLDER)
                    .enabled(live(s).native.is_none()),
            )
            .separator()
            .item(
                Item::new("close", "Close project")
                    .icon(&glyphs::CLOSE)
                    .enabled(live(s).native.is_none()),
            );
        show(s, at, origin.clone(), menu, move |s, pick, tx| match pick {
            "up" | "down" => {
                let direction = if pick == "up" {
                    ProjectMove::Up
                } else {
                    ProjectMove::Down
                };
                live_mut(s).app.move_project(&project, direction);
                tx.scroll_into_view(&origin);
            }
            "copy" => tx.copy(project.clone()),
            "remote" => crate::remote::open(s, project.clone()),
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

use super::{Item, Menu, show};
use crate::{
    Workspace,
    live::{live, live_mut},
    panels, ui,
};
use caelum_core::application::Task;
use rune::El;
use rune_kit::glyphs;

pub(crate) fn path(el: El, project: String, path: String, directory: bool, origin: String) -> El {
    el.key(&origin)
        .on_secondary_press(move |s: &mut Workspace, at, _| {
            let l = live(s);
            let owner = l.app.selected;
            let home = (l.native.is_some()
                && l.panels.screen == panels::Screen::Home
                && l.app.home.draft.project.as_ref() == Some(&project))
            .then(|| l.app.home.generation());
            let project = project.clone();
            let path = path.clone();
            let opened = l
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
                .item(
                    Item::new("reveal", "Reveal in Finder")
                        .icon(&ui::FOLDER)
                        .enabled(l.native.is_none()),
                );
            if !directory {
                menu = menu.separator().item(
                    Item::new("attach", "Add to prompt")
                        .icon(&ui::EDIT)
                        .enabled(
                            home.is_some()
                                || owner
                                    .is_some_and(|i| l.app.sessions[i].scope.project == project),
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
                "attach" if home.is_some() => {
                    let l = live_mut(s);
                    if l.panels.screen == panels::Screen::Home
                        && home == Some(l.app.home.generation())
                    {
                        l.app.home_attach_file(path);
                    }
                }
                "attach" if live(s).app.selected == owner => {
                    if let Some(i) =
                        owner.filter(|&i| live(s).app.sessions[i].scope.project == project)
                    {
                        live_mut(s).app.attach_file(i, path);
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

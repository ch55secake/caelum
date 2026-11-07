//! Native contextual actions capture the clicked object, rather than global selection.
use crate::{
    Workspace,
    live::{live, live_mut},
};
use caelum_core::application::{Attachment, Task};
use rune::{El, PopupItem, PopupMenu};

pub fn message(el: El, text: String, index: usize) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, tx| {
        let text = text.clone();
        let menu = PopupMenu::new()
            .item(PopupItem::new("copy", "Copy message"))
            .item(
                PopupItem::new("reuse", "Use in prompt")
                    .enabled(live(s).app.selected == Some(index)),
            );
        tx.popup_menu(menu, at, move |s: &mut Workspace, pick, tx| match pick {
            Some("copy") => tx.copy(text.clone()),
            Some("reuse") if live(s).app.selected == Some(index) => {
                let l = live_mut(s);
                let draft = &mut l.drafts[index];
                let combined = if draft.text.is_empty() {
                    text.clone()
                } else {
                    format!("{}\n\n{text}", draft.text)
                };
                draft.replace_text(combined);
                draft.focused = true;
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
) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, tx| {
        let owner = live(s).app.selected;
        let project = project.clone();
        let path = path.clone();
        let content = content.clone();
        let menu =
            PopupMenu::new()
                .item(PopupItem::new("copy", "Copy content"))
                .item(PopupItem::new("path", "Copy path"))
                .separator()
                .item(PopupItem::new("attach", "Add to prompt").enabled(
                    owner.is_some_and(|i| live(s).app.sessions[i].scope.project == project),
                ))
                .item(PopupItem::new("reveal", "Reveal in Finder"));
        tx.popup_menu(menu, at, move |s: &mut Workspace, pick, tx| match pick {
            Some("copy") => tx.copy(content.clone()),
            Some("path") => tx.copy(path.clone()),
            Some("attach") if live(s).app.selected == owner => {
                if let Some(i) = owner {
                    live_mut(s).app.attach(
                        i,
                        Attachment::new(path.clone(), content.clone(), range, false),
                    );
                }
            }
            Some("reveal") => live_mut(s).app.task(Task::Reveal {
                project: project.clone(),
                path: path.clone(),
            }),
            _ => {}
        });
    })
}

pub fn project(el: El, project: String) -> El {
    el.on_secondary_press(move |_: &mut Workspace, at, tx| {
        let project = project.clone();
        tx.popup_menu(
            PopupMenu::new()
                .item(PopupItem::new("new", "New session"))
                .separator()
                .item(PopupItem::new("copy", "Copy project path"))
                .item(PopupItem::new("reveal", "Reveal in Finder")),
            at,
            move |s: &mut Workspace, pick, tx| match pick {
                Some("copy") => tx.copy(project.clone()),
                Some("reveal") => live_mut(s).app.task(Task::Reveal {
                    project: project.clone(),
                    path: ".".into(),
                }),
                Some("new") => {
                    let l = live_mut(s);
                    l.app.select_project(&project);
                    l.new_session();
                }
                _ => {}
            },
        );
    })
}

pub fn session(el: El, index: usize) -> El {
    el.on_secondary_press(move |s: &mut Workspace, at, tx| {
        let session = &live(s).app.sessions[index];
        let archived = session.archived;
        let title = session.title.clone();
        tx.popup_menu(
            PopupMenu::new()
                .item(PopupItem::new("copy", "Copy session title"))
                .item(
                    PopupItem::new("refresh", "Refresh session")
                        .enabled(session.remote_id.is_some() && !session.loading),
                )
                .separator()
                .item(
                    PopupItem::new(
                        "archive",
                        if archived {
                            "Restore session"
                        } else {
                            "Archive session"
                        },
                    )
                    .enabled(
                        !session.working()
                            && !session.archiving
                            && session.queue.is_empty()
                            && session.inbox.items.is_empty(),
                    ),
                ),
            at,
            move |s: &mut Workspace, pick, tx| match pick {
                Some("copy") => tx.copy(title.clone()),
                Some("refresh") => live_mut(s).app.refresh(index),
                Some("archive") => live_mut(s).archive(index, !archived),
                _ => {}
            },
        );
    })
}

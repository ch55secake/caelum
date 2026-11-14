use crate::{
    Workspace,
    live::{live, live_mut},
    ui,
};
use rune::{El, FileDialogOptions, Tx, col, row};
use rune_kit::{Kit, glyphs};
use std::path::PathBuf;

pub(super) fn drop_files(s: &mut Workspace, paths: &[PathBuf]) {
    let l = live_mut(s);
    let Some(project) = l.app.home.draft.project.clone() else {
        l.app.home.error = Some("Choose a project before attaching files".into());
        return;
    };
    for path in paths {
        match path.strip_prefix(&project) {
            Ok(path) => l.app.home_attach_file(path.to_string_lossy().into_owned()),
            Err(_) => l.app.home.error = Some("Choose files from the selected project".into()),
        }
    }
}

pub(super) fn choose_file(s: &mut Workspace, tx: &mut Tx) {
    let generation = live(s).app.home.generation();
    tx.open_file(
        FileDialogOptions {
            title: "Attach a project file".into(),
            ..Default::default()
        },
        move |s: &mut Workspace, result, _| {
            if live(s).app.home.generation() != generation {
                return;
            }
            match result {
                Ok(Some(path)) => drop_files(s, &[path]),
                Ok(None) => {}
                Err(error) => live_mut(s).app.home.error = Some(error.to_string()),
            }
        },
    );
}

pub(super) fn view(s: &Workspace, k: &Kit) -> El {
    let l = live(s);
    let mut chips = row().w_full().flex_wrap().gap(6.0);
    for attachment in &l.app.home.draft.attachments {
        let remove = attachment.id.clone();
        let title = attachment.label();
        chips = chips.child(
            row()
                .max_w(280.0)
                .min_w(0.0)
                .center_y()
                .gap(6.0)
                .px(8.0)
                .radius(7.0)
                .fill(k.c.layer_02)
                .child(
                    ui::caption(&title, k)
                        .truncate()
                        .min_w(0.0)
                        .help(&attachment.path),
                )
                .child(
                    crate::quiet_button(&glyphs::CLOSE, &format!("Remove {title}"))
                        .on_press(move |s: &mut Workspace| {
                            live_mut(s)
                                .app
                                .home
                                .draft
                                .attachments
                                .retain(|a| a.id != remove)
                        })
                        .build(k)
                        .size(24.0, 28.0)
                        .key(format!("home-remove-{}", attachment.id)),
                ),
        );
    }
    col()
        .w_full()
        .gap(6.0)
        .children((!l.app.home.draft.attachments.is_empty()).then_some(chips))
        .children((l.app.home.attaching > 0).then(|| ui::caption("Reading attachments…", k)))
}

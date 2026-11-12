use std::hash::{DefaultHasher, Hash, Hasher};

use caelum_core::application::Workspace;
use rune::{El, TextStyle, color::hex, row, text};
use rune_kit::Kit;

pub(super) fn view(app: &Workspace, path: &str, k: &Kit) -> El {
    let root = app.group_path(path);
    let name = crate::checkouts::project_name(app, root);
    // Stable across renders and launches; linked checkouts share their project's colour.
    let mut hash = DefaultHasher::new();
    root.hash(&mut hash);
    let palette = [
        ("#6c56b0", "#a39af6"),
        ("#3568b0", "#82b4f4"),
        ("#216f68", "#72c7bf"),
        ("#406f36", "#91c788"),
        ("#875819", "#e4b56e"),
        ("#a54163", "#e79ab0"),
    ];
    let (light, dark) = palette[hash.finish() as usize % palette.len()];
    let ink = hex(if k.dark { dark } else { light }).unwrap();
    row()
        .key("project-badge")
        .size(18.0, 18.0)
        .center()
        .radius(4.0)
        .fill(k.alpha(ink, if k.dark { 0.16 } else { 0.10 }))
        .a11y_hidden()
        .child(
            text(
                name.chars()
                    .next()
                    .unwrap_or('·')
                    .to_uppercase()
                    .to_string(),
            )
            .font(TextStyle::new(10.0, 600.0).lh(12.0))
            .color(ink),
        )
}

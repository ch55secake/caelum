use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use caelum_core::application::Workspace;
use rune::{El, TextStyle, color::hex, row, text};
use rune_kit::Kit;

const PALETTE: [(&str, &str); 12] = [
    ("#6c56b0", "#a39af6"),
    ("#216f68", "#72c7bf"),
    ("#875819", "#e4b56e"),
    ("#3568b0", "#82b4f4"),
    ("#a54163", "#e79ab0"),
    ("#406f36", "#91c788"),
    ("#a14e24", "#eda477"),
    ("#236b83", "#78c5df"),
    ("#93408c", "#da99d4"),
    ("#657127", "#c0ca7c"),
    ("#4a58a9", "#9ca8f2"),
    ("#a84444", "#ee9292"),
];

#[derive(Default)]
pub(super) struct State {
    colours: HashMap<String, usize>,
}

impl State {
    pub(super) fn sync(&mut self, app: &Workspace) -> bool {
        let previous = self.colours.len();
        self.colours
            .retain(|root, _| app.group_path(root) == root && app.project_is_open(root));
        let mut changed = self.colours.len() != previous;
        let mut uses = [0; PALETTE.len()];
        for &index in self.colours.values() {
            uses[index] += 1;
        }
        for project in app
            .projects
            .iter()
            .filter(|p| app.group_path(&p.path) == p.path)
        {
            if self.colours.contains_key(&project.path) {
                continue;
            }
            // Prefer the project's hash, then an unused colour. Existing assignments
            // stay fixed as other projects open, close, or move in the sidebar.
            let start = preferred(&project.path);
            let index = (0..PALETTE.len())
                .map(|offset| (start + offset) % PALETTE.len())
                .min_by_key(|&index| uses[index])
                .unwrap();
            uses[index] += 1;
            self.colours.insert(project.path.clone(), index);
            changed = true;
        }
        changed
    }

    fn index(&self, app: &Workspace, path: &str) -> usize {
        let root = app.group_path(path);
        self.colours
            .get(root)
            .copied()
            .unwrap_or_else(|| preferred(root))
    }

    pub(super) fn view(&self, app: &Workspace, path: &str, k: &Kit) -> El {
        let name = crate::checkouts::project_name(app, app.group_path(path));
        let (light, dark) = PALETTE[self.index(app, path)];
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
}

fn preferred(root: &str) -> usize {
    let mut hash = DefaultHasher::new();
    root.hash(&mut hash);
    hash.finish() as usize % PALETTE.len()
}

#[cfg(test)]
mod tests;

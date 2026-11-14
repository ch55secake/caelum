use crate::Workspace;
use caelum_core::application::{Attachment, Provider};

pub(crate) fn scene(s: &mut Workspace, name: &str) -> bool {
    if !matches!(name, "home" | "home-empty" | "home-draft" | "home-projects") {
        return false;
    }
    crate::home::open(s);
    let l = s.live.as_mut().unwrap();
    if name == "home-empty" {
        l.app.projects.clear();
        l.app.active_project = None;
    } else {
        l.app.home_select_project("/projects/caelum".into());
        l.app.home_set_provider(Provider::Codex);
    }
    if name == "home-draft" {
        l.home.editor.replace_text(
            "Help me make a little room for a new idea.\nStart by exploring the workspace.",
        );
        l.app.home.draft.attachments.push(Attachment::new(
            "src/workspace.rs".into(),
            "fn main() {}".into(),
            None,
            false,
        ));
    }
    if name == "home-projects" {
        l.app.home.draft.project = None;
        l.app.active_project = None;
    }
    l.home.editor.focused = false;
    true
}

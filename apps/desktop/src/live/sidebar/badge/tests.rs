use super::*;
use std::collections::HashSet;

#[test]
fn open_projects_get_distinct_colours_without_recolouring_existing_roots_or_worktrees() {
    let mut app = Workspace::default();
    for name in [
        "glance",
        "singularity",
        "caelum",
        "rune",
        "website",
        "tools",
        "docs",
        "api",
        "mobile",
        "design",
        "search",
        "storage",
    ] {
        app.projects.push(caelum_protocol::ProjectInfo {
            name: name.into(),
            path: format!("/projects/{name}"),
        });
    }
    let mut state = State::default();
    assert!(state.sync(&app));
    assert_eq!(
        state.colours.values().collect::<HashSet<_>>().len(),
        app.projects.len()
    );
    assert_ne!(
        state.index(&app, "/projects/glance"),
        state.index(&app, "/projects/singularity")
    );
    let original = state.colours.clone();
    app.projects.reverse();
    assert!(!state.sync(&app));
    assert_eq!(state.colours, original);

    let linked = "/worktrees/glance/review";
    app.projects.push(caelum_protocol::ProjectInfo {
        name: "Review".into(),
        path: linked.into(),
    });
    assert!(state.sync(&app));
    app.project_roots
        .insert(linked.into(), "/projects/glance".into());
    assert!(state.sync(&app));
    assert_eq!(state.colours, original);
    assert_eq!(
        state.index(&app, linked),
        state.index(&app, "/projects/glance")
    );

    app.close_project("/projects/rune");
    assert!(state.sync(&app));
    assert!(!state.colours.contains_key("/projects/rune"));
    app.projects.push(caelum_protocol::ProjectInfo {
        name: "Fresh".into(),
        path: "/projects/fresh".into(),
    });
    assert!(state.sync(&app));
    assert_eq!(
        state.colours.values().collect::<HashSet<_>>().len(),
        PALETTE.len()
    );
    for (path, colour) in &original {
        if app.project_is_open(path) {
            assert_eq!(state.colours[path], *colour);
        }
    }
    app.projects.clear();
    assert!(state.sync(&app));
    assert!(state.colours.is_empty());
    assert!(!state.sync(&app));
}

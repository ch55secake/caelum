use super::*;

#[test]
fn managed_worktrees_keep_parent_groups_in_navigation_search_and_actions() {
    use caelum_core::application::{Command, Task};
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("sidebar-worktrees-projects", size);
        for (parent, child) in [
            ("/projects/caelum", "/projects/caelum-native"),
            ("/projects/rune", "/worktrees/rune/key-navigation"),
        ] {
            let group = element(&rt, &crate::code_view::source_key(parent, "project-group"));
            let row = element(&rt, &crate::code_view::source_key(child, "checkout"));
            assert!(row.path.starts_with(&group.path));
            assert!(
                row.rect[0]
                    > element(&rt, &crate::code_view::source_key(parent, "project")).rect[0] + 10.0
            );
        }
        assert!(
            rt.find(&crate::code_view::source_key(
                "/scratch/external-review",
                "checkout"
            ))
            .is_none()
        );
        assert!(
            rt.find(&crate::code_view::source_key(
                "/projects/caelum",
                "checkout"
            ))
            .is_none()
        );
        click(
            &mut rt,
            &crate::code_view::source_key("/projects/caelum", "manage-worktrees"),
        );
        let rows = element(&rt, "worktrees-scroll");
        assert!(
            element(
                &rt,
                &crate::code_view::source_key("/projects/caelum-native", "manage-checkout")
            )
            .path
            .starts_with(&rows.path)
        );
        assert!(
            !element(
                &rt,
                &crate::code_view::source_key("/projects/caelum", "manage-checkout")
            )
            .path
            .starts_with(&rows.path)
        );
        assert!(
            rt.find(&crate::code_view::source_key(
                "/worktrees/rune/key-navigation",
                "manage-checkout"
            ))
            .is_none()
        );
        assert!(
            rt.find(&crate::code_view::source_key(
                "/scratch/external-review",
                "manage-checkout"
            ))
            .is_none()
        );
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .app
                .select_project("/projects/rune")
        });
        advance(&mut rt, 0.2);
        click(&mut rt, "manager-new-worktree");
        fill(&mut rt, "worktree-branch", "work/owned-by-caelum");
        key(&mut rt, Key::Enter, Mods::default());
        assert!(
            rt.state
                .live
                .as_mut()
                .unwrap()
                .app
                .commands()
                .iter()
                .any(|(_, c)| matches!(c,
            Command::Task(Task::CreateWorktree { project, .. }) if project == "/projects/caelum"))
        );

        let mut rt = design_runtime("sidebar-worktrees-projects", size);
        rt.update(|s, _| crate::panels::open_launcher(s));
        advance(&mut rt, 0.3);
        for (parent, child) in [
            ("/projects/caelum", "/projects/caelum-native"),
            ("/projects/rune", "/worktrees/rune/key-navigation"),
        ] {
            let group = element(&rt, &crate::code_view::source_key(parent, "launch-group"));
            assert!(
                element(&rt, &crate::code_view::source_key(child, "launch"))
                    .path
                    .starts_with(&group.path)
            );
        }
        assert!(
            rt.find(&crate::code_view::source_key(
                "/scratch/external-review",
                "launch"
            ))
            .is_none()
        );
        fill(&mut rt, "launch-search", "key-navigation");
        assert!(
            rt.find(&crate::code_view::source_key(
                "/projects/rune",
                "launch-group"
            ))
            .is_some()
        );
        assert!(
            rt.find(&crate::code_view::source_key(
                "/projects/caelum",
                "launch-group"
            ))
            .is_none()
        );
        click(
            &mut rt,
            &crate::code_view::source_key("/worktrees/rune/key-navigation", "launch"),
        );
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .active_project
                .as_deref(),
            Some("/worktrees/rune/key-navigation")
        );
    }
    let rt = design_runtime("worktrees-empty", (800.0, 620.0));
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("No Caelum worktrees yet"))
    );
    assert!(element(&rt, "manager-new-worktree").visible[3] >= 30.0);
    assert!(rt.elements().iter().any(|e| e.text.as_deref()
        == Some("Create a worktree to work on caelum in parallel.")
        && e.visible[3] > 10.0));
    let rt = design_runtime("worktrees-error", (800.0, 620.0));
    assert!(
        element(
            &rt,
            &crate::code_view::source_key(
                "/Users/oscar/.caelum/worktrees/caelum-7d0420ab/feat-native",
                "manage-checkout"
            )
        )
        .visible[3]
            >= 57.0
    );
}

#[test]
fn sidebar_worktrees_switch_exact_checkouts_and_keep_background_work() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("sidebar-worktrees", size);
        let root = "/projects/caelum";
        let linked = "/projects/caelum-native";
        let main_key = crate::code_view::source_key(root, "project");
        let linked_key = crate::code_view::source_key(linked, "checkout");
        let linked_path = element(&rt, &linked_key).path;
        assert!(rt.elements().iter().any(|e| e.path.starts_with(&linked_path)
            && e.text.as_deref() == Some("1 working")));
        fill(&mut rt, "live-draft-0", "Keep the main draft");
        click(&mut rt, &linked_key);
        let index = rt.state.live.as_ref().unwrap().app.selected.unwrap();
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .active_project
                .as_deref(),
            Some(linked)
        );
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Keep the main draft"
        );
        click(&mut rt, &main_key);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
        assert!(rt.state.live.as_ref().unwrap().app.sessions[index].working());
        assert!(!rt.state.live.as_ref().unwrap().app.sessions[index].stopping);
        assert!(element(&rt, "sidebar-scroll").visible[3] >= 80.0);
        let toggle = crate::code_view::source_key(root, "worktrees-toggle");
        click(&mut rt, &toggle);
        assert!(rt.find(&linked_key).is_none());
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .preferences
                .worktree_expanded
                .get(root),
            Some(&false)
        );
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.3);
        assert!(rt.find(&linked_key).is_some());
        let next_key = crate::code_view::source_key("/projects/caelum-providers", "checkout");
        click(&mut rt, &next_key);
        key(&mut rt, Key::Up, Mods::default());
        advance(&mut rt, 0.2);
        assert!(element(&rt, &linked_key).focused);
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(index));
    }
}

#[test]
fn selecting_a_session_keeps_its_worktree_group_collapsed_by_default() {
    let mut rt = design_runtime("sidebar-worktrees-projects", (800.0, 620.0));
    let root = "/projects/caelum";
    let selected_worktree = crate::code_view::source_key("/projects/caelum-native", "checkout");
    let other_expanded_worktree =
        crate::code_view::source_key("/worktrees/rune/key-navigation", "checkout");

    assert!(rt.find(&selected_worktree).is_some());
    assert!(rt.find(&other_expanded_worktree).is_some());
    click(&mut rt, "live-session-1");

    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
    assert_eq!(
        element(&rt, &crate::code_view::source_key(root, "worktrees-toggle")).expanded,
        Some(false)
    );
    assert!(rt.find(&selected_worktree).is_none());
    assert!(rt.find(&other_expanded_worktree).is_some());
}

#[test]
fn selecting_a_worktree_session_does_not_reopen_its_collapsed_group() {
    let mut rt = design_runtime("sidebar-worktrees-projects", (800.0, 620.0));
    let root = "/projects/caelum";
    let selected_worktree = crate::code_view::source_key("/projects/caelum-native", "checkout");
    let other_expanded_worktree =
        crate::code_view::source_key("/worktrees/rune/key-navigation", "checkout");

    assert!(rt.find(&selected_worktree).is_some());
    rt.update(|_, tx| tx.scroll_to_y("sidebar-scroll", 1000.0));
    advance(&mut rt, 0.2);
    click(&mut rt, "live-session-3");

    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.selected, Some(3));
    assert_eq!(
        live.app.sessions[3].scope.project,
        "/projects/caelum-native"
    );
    assert_eq!(
        element(&rt, &crate::code_view::source_key(root, "worktrees-toggle")).expanded,
        Some(false)
    );
    assert!(rt.find(&selected_worktree).is_none());
    assert!(rt.find(&other_expanded_worktree).is_some());
}

#[test]
fn worktree_management_keeps_its_project_and_missing_rows_cannot_navigate() {
    let mut rt = design_runtime("sidebar-worktrees-many", (800.0, 620.0));
    let root = "/projects/caelum";
    let missing = crate::code_view::source_key("/projects/caelum-task-11", "checkout");
    assert!(!element(&rt, &missing).focusable);
    let last = crate::code_view::source_key("/projects/caelum-task-10", "checkout");
    click(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum-native", "checkout"),
    );
    let selected = rt.state.live.as_ref().unwrap().app.selected;
    for _ in 0..12 {
        key(&mut rt, Key::Down, Mods::default());
    }
    advance(&mut rt, 0.3);
    let row = element(&rt, &last);
    assert!(row.focused && row.visible[3] >= 41.0, "{row:?}");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, selected);
    rt.update(|s, tx| {
        crate::checkouts::manage(s, root.into());
        tx.scroll_to_y("projects-scroll", 0.0);
        let l = s.live.as_mut().unwrap();
        l.app.projects.push(caelum_protocol::ProjectInfo {
            path: "/other".into(),
            name: "Other".into(),
        });
        l.app.select_project("/other");
        l.sync_editors();
    });
    advance(&mut rt, 0.3);
    assert_eq!(
        rt.state.live.as_ref().unwrap().checkouts.project.as_deref(),
        Some(root)
    );
    assert!(
        rt.find(&crate::code_view::source_key(root, "manage-checkout"))
            .is_some()
    );
    click(
        &mut rt,
        &crate::code_view::source_key(root, "manage-checkout"),
    );
    assert_eq!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .active_project
            .as_deref(),
        Some(root)
    );
    assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
}

#[test]
fn worktree_discovery_loads_background_projects_once_and_refreshes_without_navigation() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = design_runtime("sidebar-projects", (1280.0, 1000.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.commands();
        assert!(l.checkouts.sync(&mut l.app));
        let commands = l.app.commands();
        assert_eq!(commands.len(), 4);
        for (id, command) in commands {
            assert!(matches!(command, Command::Task(Task::Worktrees { .. })));
            l.app.apply(Event::Completed(
                id,
                Ok(Reply::Task(TaskReply::Worktrees(None))),
            ));
        }
        assert!(
            !l.checkouts.sync(&mut l.app),
            "non-Git projects must not retry every frame"
        );
        assert!(l.app.commands().is_empty());
    });
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    let mut rt = design_runtime("sidebar-worktrees", (1280.0, 1000.0));
    click(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum", "refresh-worktrees"),
    );
    assert!(rt.state.live.as_mut().unwrap().app.commands().iter().any(|(_, c)| matches!(c, Command::Task(Task::Worktrees { project, .. }) if project == "/projects/caelum")));
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
}

#[test]
fn worktree_labels_can_be_edited_from_the_checkout_list() {
    use caelum_core::application::{Command, Task};
    let mut rt = design_runtime("worktrees", (800.0, 620.0));
    let path = "/Users/oscar/.caelum/worktrees/caelum-7d0420ab/feat-native";
    let rename_key = crate::code_view::source_key(path, "rename-worktree");
    let name_key = crate::code_view::source_key(path, "rename-worktree-name");
    let save_key = crate::code_view::source_key(path, "save-worktree-name");
    assert!(element(&rt, &rename_key).visible[3] > 0.0);
    click(&mut rt, &rename_key);
    fill(&mut rt, &name_key, "Fix checkout picker");
    click(&mut rt, &save_key);

    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, command)| matches!(
        command,
        Command::Task(Task::RenameWorktree { path, name, .. })
            if path == "/Users/oscar/.caelum/worktrees/caelum-7d0420ab/feat-native"
                && name == "Fix checkout picker"
    )));
}

#[test]
fn worktree_form_searches_bases_and_captures_project_provider_and_branch() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = design_runtime("worktree-create", (800.0, 620.0));
    assert!(element(&rt, "worktree-branch").focused);
    rt.paste("feat/café");
    advance(&mut rt, 0.1);
    click(&mut rt, "worktree-base-trigger");
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.1);
    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.overlay,
        Some(crate::panels::Screen::CreateWorktree)
    );
    assert!(element(&rt, "worktree-base-trigger").focused);
    click(&mut rt, "worktree-base-trigger");
    fill(&mut rt, "worktree-base-search", "native");
    assert_eq!(
        element(&rt, "option0").label.as_deref(),
        Some("feat/native")
    );
    click(&mut rt, "option0");
    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.overlay,
        Some(crate::panels::Screen::CreateWorktree)
    );
    click(&mut rt, "worktree-branch");
    key(&mut rt, Key::Enter, Mods::default());
    let (request, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
        .expect("Enter submits the form");
    assert!(
        matches!(command, Command::Task(Task::CreateWorktree { project, branch, base, .. })
        if project == "/projects/caelum" && branch == "feat/café" && base == "feat/native")
    );
    let mut open = None;
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.panels.launch_provider = caelum_core::application::Provider::Codex;
        l.apply(Event::Completed(request, Ok(Reply::Task(TaskReply::Worktree(serde_json::from_value(serde_json::json!({
            "path":"/linked","project":{"path":"/linked","name":"Linked"},"branch":"feat/café","main":false,"locked":null,"prunable":null,"available":true
        })).unwrap())))));
        open = Some(l.app.commands().into_iter().find(|(_, c)| matches!(c, Command::Open(_))).unwrap().0);
    });
    advance(&mut rt, 0.1);
    click(&mut rt, "worktree-branch");
    key(&mut rt, Key::Enter, Mods::default());
    assert!(
        !rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::Task(Task::CreateWorktree { .. })))
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            open.unwrap(),
            Ok(Reply::Project(caelum_protocol::ProjectInfo {
                path: "/linked".into(),
                name: "Linked".into(),
            })),
        ));
    });
    advance(&mut rt, 0.3);
    let l = rt.state.live.as_ref().unwrap();
    assert!(l.panels.overlay.is_none());
    assert_eq!(
        l.app.sessions[l.app.selected.unwrap()].scope.provider,
        caelum_core::application::Provider::OpenCode
    );
    assert!(element(&rt, &format!("live-draft-{}", l.app.selected.unwrap())).focused);
}

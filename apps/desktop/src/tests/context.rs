use super::*;

#[test]
fn context_menus_keep_background_session_ownership_and_keyboard_focus() {
    let mut rt = live_runtime_size((800.0, 620.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[0].conversation.working = true;
        l.app.sessions[1].remote_id = Some("background".into());
        l.app.sessions[1].loaded = true;
        l.app.sessions[1].title = "Background session".into();
    });
    secondary(&mut rt, "live-session-1");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    choose_context(&mut rt, "Copy session title");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("Background session")
    );
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    assert!(element(&rt, "session-actions-1").focused);
    key(
        &mut rt,
        Key::Function(10),
        Mods {
            shift: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.2);
    assert!(crate::context::is_open(&rt.state));
    key(&mut rt, Key::End, Mods::default());
    assert!(context_item(&rt, "Archive session").focused);
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.3);
    assert!(element(&rt, "session-actions-1").focused);
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    secondary(&mut rt, "live-session-0");
    assert!(!context_item(&rt, "Archive session").focusable);
    let archive = context_item(&rt, "Archive session").key.unwrap();
    click(&mut rt, &archive);
    assert!(crate::context::is_open(&rt.state));
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].archived);
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.3);
    secondary(&mut rt, "live-session-1");
    choose_context(&mut rt, "Refresh session");
    let l = rt.state.live.as_mut().unwrap();
    let commands = l.app.commands();
    assert!(commands.iter().any(|(_, c)| matches!(c, caelum_core::application::Command::Resume(key) if key.id == "background")));
    assert!(commands.iter().any(|(_, c)| matches!(c, caelum_core::application::Command::RegenerateTitle(key) if key.id == "background")));
    assert_eq!(l.app.selected, Some(0));
}

#[test]
fn context_editor_preserves_unicode_selection_undo_and_paste_ownership() {
    let mut rt = live_runtime();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    fill(&mut rt, "live-draft-0", "Keep café ☁️ safe");
    rt.update(|s, _| s.live.as_mut().unwrap().drafts[0].set_selection(5, "Keep café ☁️".len()));
    secondary(&mut rt, "live-draft-0");
    choose_context(&mut rt, "Cut");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("café ☁️")
    );
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Keep  safe");
    assert!(element(&rt, "live-draft-0").focused);
    secondary(&mut rt, "live-draft-0");
    choose_context(&mut rt, "Undo");
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep café ☁️ safe"
    );
    secondary(&mut rt, "live-draft-0");
    choose_context(&mut rt, "Redo");
    secondary(&mut rt, "live-draft-0");
    choose_context(&mut rt, "Paste");
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep café ☁️ safe"
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0].draft,
        "Keep café ☁️ safe"
    );
    secondary(&mut rt, "live-draft-0");
    choose_context(&mut rt, "Select all");
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].selected(),
        "Keep café ☁️ safe"
    );
    secondary(&mut rt, "live-draft-0");
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
    advance(&mut rt, 0.3);
    assert!(!crate::context::is_open(&rt.state));
    assert!(rt.find("context-menu").is_none());
    assert!(rt.state.live.as_ref().unwrap().drafts[1].text.is_empty());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep café ☁️ safe"
    );
}

#[test]
fn context_menus_stay_in_bounds_and_dismiss_before_parent_overlays() {
    for dark in [false, true] {
        let mut rt = design_runtime("models", (800.0, 620.0));
        rt.update(|s, _| s.dark = Some(dark));
        fill(&mut rt, "model-search", "model");
        secondary(&mut rt, "model-search");
        let menu = element(&rt, "context-menu");
        assert!(
            menu.rect
                .iter()
                .zip(menu.visible)
                .all(|(a, b)| (*a - b).abs() < 1.0),
            "{menu:?}"
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(rt.state.live.as_ref().unwrap().models.is_open());
        assert!(element(&rt, "model-search").focused);
        secondary(&mut rt, "model-search");
        key(&mut rt, Key::Tab, Mods::default());
        advance(&mut rt, 0.3);
        assert!(rt.state.live.as_ref().unwrap().models.is_open());
        assert!(element(&rt, "model-search").focused);
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        rt.secondary_down(795.0, 615.0);
        rt.secondary_up(795.0, 615.0);
        advance(&mut rt, 0.3);
        let menu = element(&rt, "context-menu");
        assert!(menu.rect[0] >= 0.0 && menu.rect[1] >= 0.0);
        assert!(menu.rect[0] + menu.rect[2] <= 800.0 && menu.rect[1] + menu.rect[3] <= 620.0);
        click(&mut rt, "nav-settings");
        assert!(!crate::context::is_open(&rt.state));
        assert!(
            rt.state.live.as_ref().unwrap().panels.overlay.is_none(),
            "outside click only dismisses the menu"
        );
    }
}

#[test]
fn context_message_and_file_actions_capture_the_clicked_content() {
    let mut rt = design_runtime("messages", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    secondary(&mut rt, "user-bubble-0-opencode-formatted");
    choose_context(&mut rt, "Copy message");
    assert!(clipboard.text().unwrap().contains("**session owner**"));
    let original = rt.state.live.as_ref().unwrap().drafts[0].text.clone();
    secondary(&mut rt, "user-bubble-0-opencode-formatted");
    choose_context(&mut rt, "Use in prompt");
    assert!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .starts_with(&original)
    );
    assert!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .contains("**session owner**")
    );
    assert!(element(&rt, "live-draft-0").focused);

    let mut rt = design_runtime("files", (1280.0, 1000.0));
    rt.use_clipboard(&clipboard);
    let project = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .active_project
        .clone()
        .unwrap();
    let source = crate::code_view::source_key(&project, "Cargo.toml");
    secondary(&mut rt, &source);
    choose_context(&mut rt, "Copy full path");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some(format!("{project}/Cargo.toml").as_str())
    );
    secondary(&mut rt, &source);
    choose_context(&mut rt, "Add to prompt");
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.app.sessions[0].attachments[0].path, "Cargo.toml");
    assert!(
        l.app.sessions[0].attachments[0]
            .content
            .contains("[package]")
    );
    assert!(l.app.sessions[1].attachments.is_empty());
}

#[test]
fn context_paths_preserve_the_open_file_and_project_until_chosen() {
    let mut rt = design_runtime("files-tabs", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let project = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .active_project
        .clone()
        .unwrap();
    let tab = format!(
        "file-tab-{}",
        crate::code_view::source_key(&project, "README.md")
    );
    secondary(&mut rt, &tab);
    choose_context(&mut rt, "Copy relative path");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("README.md")
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.services.checkouts[&project]
            .selected_file
            .as_deref(),
        Some("Cargo.toml")
    );
    secondary(&mut rt, &tab);
    choose_context(&mut rt, "Close file");
    let checkout = &rt.state.live.as_ref().unwrap().app.services.checkouts[&project];
    assert!(!checkout.tabs.iter().any(|p| p == "README.md"));
    assert_eq!(checkout.selected_file.as_deref(), Some("Cargo.toml"));
    secondary(&mut rt, "file-tree-Cargo.toml");
    choose_context(&mut rt, "Copy full path");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some(format!("{project}/Cargo.toml").as_str())
    );

    let mut rt = live_runtime();
    rt.use_clipboard(&clipboard);
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .app
            .projects
            .push(caelum_protocol::ProjectInfo {
                name: "Other project".into(),
                path: "/other".into(),
            })
    });
    fill(&mut rt, "live-draft-0", "Keep my project draft");
    let other = crate::code_view::source_key("/other", "project");
    reveal(&mut rt, &other);
    secondary(&mut rt, &other);
    choose_context(&mut rt, "Copy project path");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("/other")
    );
    assert_eq!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .active_project
            .as_deref(),
        Some("/project")
    );
    secondary(&mut rt, &other);
    choose_context(&mut rt, "New session");
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(
        l.app.sessions[l.app.selected.unwrap()].scope.project,
        "/other"
    );
    assert_eq!(l.drafts[0].text, "Keep my project draft");
}

#[test]
fn context_worktrees_delete_the_clicked_checkout_and_surface_failures() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let parent = "/projects/rune";
    let path = "/worktrees/rune/key-navigation";
    let directory = format!("{path}/app");
    for surface in ["checkout", "manage-checkout", "launch"] {
        let mut rt = design_runtime("sidebar-worktrees-projects", (800.0, 620.0));
        fill(&mut rt, "live-draft-0", "Keep the other project's draft");
        rt.update(|s, _| {
            let app = &mut s.live.as_mut().unwrap().app;
            // A subproject's navigation path differs from the Git worktree root.
            app.projects
                .iter_mut()
                .find(|p| p.path == path)
                .unwrap()
                .path = directory.clone();
            app.project_roots.insert(directory.clone(), parent.into());
            app.services
                .checkouts
                .get_mut(parent)
                .unwrap()
                .worktrees
                .as_mut()
                .unwrap()
                .worktrees
                .iter_mut()
                .find(|tree| tree.path == path)
                .unwrap()
                .project
                .path = directory.clone();
            app.commands();
        });
        match surface {
            "manage-checkout" => rt.update(|s, _| crate::checkouts::manage(s, parent.into())),
            "launch" => {
                rt.update(|s, _| crate::panels::open_launcher(s));
                advance(&mut rt, 0.2);
                fill(&mut rt, "launch-search", "key-navigation");
            }
            _ => rt.update(|_, tx| tx.scroll_to_y("projects-scroll", 1000.0)),
        }
        rt.update(|s, _| {
            let app = &mut s.live.as_mut().unwrap().app;
            for (request, command) in app.commands() {
                if let Command::Task(Task::Worktrees { project, .. }) = command {
                    let listing = app.services.worktrees(&project).cloned();
                    app.apply(Event::Completed(
                        request,
                        Ok(Reply::Task(TaskReply::Worktrees(listing))),
                    ));
                }
            }
        });
        advance(&mut rt, 0.3);
        let row = crate::code_view::source_key(&directory, surface);
        secondary(&mut rt, &row);
        let menu = element(&rt, "context-menu");
        assert!(
            menu.rect
                .iter()
                .zip(menu.visible)
                .all(|(a, b)| (*a - b).abs() < 1.0)
        );
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(element(&rt, &row).focused);
        key(
            &mut rt,
            Key::Function(10),
            Mods {
                shift: true,
                ..Default::default()
            },
        );
        advance(&mut rt, 0.2);
        key(&mut rt, Key::End, Mods::default());
        assert!(context_item(&rt, "Delete worktree").focused);
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.3);
        assert!(!crate::context::is_open(&rt.state));
        let remove = |rt: &mut Runtime<Workspace>| {
            let commands = rt.state.live.as_mut().unwrap().app.commands();
            let removals: Vec<_> = commands
                .into_iter()
                .filter(|(_, c)| matches!(c, Command::Task(Task::RemoveWorktree { .. })))
                .collect();
            assert_eq!(removals.len(), 1);
            assert!(
                matches!(&removals[0].1, Command::Task(Task::RemoveWorktree { project, path: target })
                if project == parent && target == path)
            );
            removals[0].0
        };
        let request = remove(&mut rt);
        let failure = "Worktree contains modified or untracked files";
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .app
                .apply(Event::Completed(request, Err(failure.into())))
        });
        advance(&mut rt, 0.3);
        assert!(rt.find(&row).is_some());
        assert!(rt.elements().iter().any(|e| !e.exiting
            && e.text.as_deref()
                == Some(if surface == "checkout" {
                    "Couldn't update worktrees"
                } else {
                    failure
                })));
        secondary(&mut rt, &row);
        choose_context(&mut rt, "Delete worktree");
        let request = remove(&mut rt);
        rt.update(|s, _| {
            let app = &mut s.live.as_mut().unwrap().app;
            let tree = app
                .managed_worktrees(parent)
                .find(|tree| tree.path == path)
                .unwrap()
                .clone();
            app.apply(Event::Completed(
                request,
                Ok(Reply::Task(TaskReply::Worktree(tree))),
            ));
        });
        advance(&mut rt, 0.3);
        assert!(rt.find(&row).is_none());
        assert!(
            rt.find(&crate::code_view::source_key(parent, "worktree-group"))
                .is_none()
        );
        let l = rt.state.live.as_mut().unwrap();
        assert_eq!(l.app.active_project.as_deref(), Some("/projects/caelum"));
        assert_eq!(l.app.selected, Some(0));
        assert_eq!(l.drafts[0].text, "Keep the other project's draft");
        assert_eq!(l.app.managed_worktrees("/projects/caelum").count(), 2);
        assert!(l.app.commands().iter().any(|(_, c)| matches!(c,
            Command::Task(Task::Worktrees { project, .. }) if project == parent)));
    }
}

#[test]
fn context_menu_can_rename_the_clicked_worktree() {
    use caelum_core::application::{Command, Task};
    let parent = "/projects/rune";
    let path = "/worktrees/rune/key-navigation";
    let row = crate::code_view::source_key(path, "checkout");
    let mut rt = design_runtime("sidebar-worktrees-projects", (800.0, 620.0));
    rt.update(|_, tx| tx.scroll_to_y("projects-scroll", 1000.0));
    advance(&mut rt, 0.2);
    secondary(&mut rt, &row);
    choose_context(&mut rt, "Rename worktree");
    advance(&mut rt, 0.2);

    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.overlay,
        Some(crate::panels::Screen::Worktrees)
    );
    assert!(crate::worktree::is_renaming(&rt.state, path));
    let name_key = crate::code_view::source_key(path, "rename-worktree-name");
    let save_key = crate::code_view::source_key(path, "save-worktree-name");
    fill(&mut rt, &name_key, "Key navigation fix");
    click(&mut rt, &save_key);

    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, command)| matches!(
        command,
        Command::Task(Task::RenameWorktree { project, path: target, name, .. })
            if project == parent && target == path && name == "Key navigation fix"
    )));
}

#[test]
fn context_worktree_deletion_rechecks_locks_mutations_and_catalog_changes() {
    use caelum_core::application::{Command, Task};
    let parent = "/projects/rune";
    let path = "/worktrees/rune/key-navigation";
    let row = crate::code_view::source_key(path, "checkout");
    for changed in ["locked", "busy", "removed"] {
        let mut rt = design_runtime("sidebar-worktrees-projects", (1280.0, 1000.0));
        reveal(&mut rt, &row);
        secondary(&mut rt, &row);
        assert!(context_item(&rt, "Delete worktree").focusable);
        rt.update(|s, _| {
            let app = &mut s.live.as_mut().unwrap().app;
            if changed == "busy" {
                app.task(Task::RemoveWorktree {
                    project: parent.into(),
                    path: path.into(),
                });
            } else {
                let trees = &mut app
                    .services
                    .checkouts
                    .get_mut(parent)
                    .unwrap()
                    .worktrees
                    .as_mut()
                    .unwrap()
                    .worktrees;
                if changed == "locked" {
                    trees.iter_mut().find(|t| t.path == path).unwrap().locked =
                        Some("Keep this worktree".into());
                } else {
                    trees.retain(|t| t.path != path);
                }
            }
            app.commands();
        });
        choose_context(&mut rt, "Delete worktree");
        assert!(
            !rt.state
                .live
                .as_mut()
                .unwrap()
                .app
                .commands()
                .iter()
                .any(|(_, c)| matches!(c, Command::Task(Task::RemoveWorktree { .. })))
        );
        if changed != "removed" {
            secondary(&mut rt, &row);
            assert!(!context_item(&rt, "Delete worktree").focusable);
        }
    }
    let mut rt = design_runtime("sidebar-worktrees", (800.0, 620.0));
    reveal(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum-native", "checkout"),
    );
    secondary(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum-native", "checkout"),
    );
    choose_context(&mut rt, "Delete worktree");
    assert!(
        !rt.state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .iter()
            .any(|(_, c)| matches!(c, Command::Task(Task::RemoveWorktree { .. })))
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| !e.exiting && e.text.as_deref() == Some("Couldn't update worktrees"))
    );
    assert!(rt.state.live.as_ref().unwrap().app.sessions.iter().any(
        |session| session.scope.project == "/projects/caelum-native"
            && session.working()
            && !session.draft.is_empty()
    ));
}

use rune::{Env, Key, KeyEvent, Mods, Runtime, runtime::ElementInfo};

use super::{Workspace, spec};

fn runtime(size: (f32, f32)) -> Runtime<Workspace> {
    let app = spec();
    let mut rt = Runtime::new((app.init)(), app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

fn advance(rt: &mut Runtime<Workspace>, seconds: f64) {
    for _ in 0..(seconds * 60.0).ceil() as usize {
        rt.frame(1.0 / 60.0);
        if let Some(focused) = crate::live::presented_focus(rt) {
            rt.update(|s, _| s.live.as_mut().unwrap().focus_key = focused);
        }
    }
}

fn element(rt: &Runtime<Workspace>, key: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {key}"))
}

fn click(rt: &mut Runtime<Workspace>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    advance(rt, 0.1);
}

fn key(rt: &mut Runtime<Workspace>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1.0 / 60.0);
}

fn fill(rt: &mut Runtime<Workspace>, field: &str, value: &str) {
    click(rt, field);
    key(
        rt,
        Key::Command('a'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    rt.paste(value);
    advance(rt, 0.1);
}

fn secondary(rt: &mut Runtime<Workspace>, target: &str) {
    let (x, y) = rt
        .find(target)
        .unwrap_or_else(|| panic!("missing {target}"));
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    advance(rt, 0.2);
    assert!(rt.popup_menu().is_none(), "context menus are app-owned");
    assert!(rt.find("context-menu").is_some());
}

fn context_item(rt: &Runtime<Workspace>, label: &str) -> ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| {
            !e.exiting
                && e.role == "menuitem"
                && e.path.contains("context-menu")
                && e.label.as_deref() == Some(label)
        })
        .unwrap_or_else(|| panic!("missing context action {label}"))
}

fn choose_context(rt: &mut Runtime<Workspace>, label: &str) {
    let key = context_item(rt, label).key.unwrap();
    click(rt, &key);
    advance(rt, 0.3);
    assert!(!crate::context::is_open(&rt.state));
}

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
    assert!(l.app.commands().iter().any(|(_, c)| matches!(c, caelum_core::application::Command::Resume(key) if key.id == "background")));
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
fn session_popover_roves_without_selecting_its_background_owner() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[1].remote_id = Some("background".into());
        l.app.sessions[1].loaded = true;
        l.app.sessions[0].conversation.working = true;
    });
    let (x, y) = rt.find("session-actions-1").unwrap();
    rt.pointer_move(x, y);
    advance(&mut rt, 0.2);
    click(&mut rt, "session-actions-1");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    let popup = element(&rt, "session-menu");
    assert!(
        popup
            .rect
            .iter()
            .zip(popup.visible)
            .all(|(a, b)| (*a - b).abs() < 1.0)
    );
    key(&mut rt, Key::Down, Mods::default());
    advance(&mut rt, 0.1);
    assert!(element(&rt, "archive-session").focused || element(&rt, "refresh-history").focused);
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, "session-actions-1").focused);
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    click(&mut rt, "session-actions-1");
    click(&mut rt, "refresh-history");
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, c)| matches!(c, caelum_core::application::Command::Resume(key) if key.id == "background")));
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
}

#[test]
fn archive_toast_pauses_for_presented_keyboard_focus_then_expires() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| s.live.as_mut().unwrap().archive(1, true));
    advance(&mut rt, 0.3);
    for _ in 0..40 {
        key(&mut rt, Key::Tab, Mods::default());
        advance(&mut rt, 0.05);
        if element(&rt, "undo-archive").focused {
            break;
        }
    }
    assert!(element(&rt, "undo-archive").focused);
    let now = caelum_core::application::presentation::now();
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert_eq!(l.focus_key.as_deref(), Some("undo-archive"));
        assert!(!l.tick_archive(now + 30_000));
    });
    click(&mut rt, "live-draft-0");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert!(l.tick_archive(now + 39_000));
        assert!(l.app.sessions[1].archived);
    });
    advance(&mut rt, 0.2);
    assert!(
        !rt.elements()
            .iter()
            .any(|e| !e.exiting && e.key.as_deref() == Some("archive-toast"))
    );
}

fn design_runtime(scene: &str, size: (f32, f32)) -> Runtime<Workspace> {
    let app = spec();
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, scene);
    let mut rt = Runtime::new(state, app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    advance(&mut rt, 0.4);
    rt
}

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
fn context_worktree_deletion_rechecks_locks_mutations_and_catalog_changes() {
    use caelum_core::application::{Command, Task};
    let parent = "/projects/rune";
    let path = "/worktrees/rune/key-navigation";
    let row = crate::code_view::source_key(path, "checkout");
    for changed in ["locked", "busy", "removed"] {
        let mut rt = design_runtime("sidebar-worktrees-projects", (1280.0, 1000.0));
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
fn populated_sidebar_shares_space_and_scrolls_projects_and_pull_requests_independently() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("sidebar-projects", size);
        let projects = element(&rt, "projects-scroll");
        let requests = element(&rt, "requests-scroll");
        assert!(projects.visible[3] > 130.0, "{projects:?}");
        assert!(requests.visible[3] > 130.0, "{requests:?}");
        assert!(element(&rt, "sidebar-scroll").visible[3] >= 100.0);
        assert!(requests.rect[1] + requests.rect[3] <= size.1);
        let owner = rt.state.live.as_ref().unwrap().app.selected;
        let first = crate::code_view::source_key("/projects/caelum", "project");
        click(&mut rt, &first);
        for _ in 0..3 {
            key(&mut rt, Key::Down, Mods::default());
        }
        advance(&mut rt, 0.3);
        let last = element(
            &rt,
            &crate::code_view::source_key("/projects/tools", "project"),
        );
        assert!(last.focused && last.visible[3] >= 69.0, "{last:?}");
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, owner);
        click(&mut rt, "pull-request-101");
        for _ in 0..5 {
            key(&mut rt, Key::Down, Mods::default());
        }
        advance(&mut rt, 0.3);
        let last = element(&rt, "pull-request-106");
        assert!(last.focused && last.visible[3] >= 51.0, "{last:?}");
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .panels
                .review
                .as_ref()
                .unwrap()
                .1,
            "https://github.com/ch55secake/caelum/pull/106"
        );
    }
}

#[test]
fn closing_projects_by_keyboard_preserves_drafts_and_keeps_navigation_usable() {
    use caelum_core::application::{Command, Event, Reply};
    let mut rt = design_runtime("sidebar-projects", (800.0, 620.0));
    fill(&mut rt, "live-draft-0", "Retain this project draft café");
    let project = crate::code_view::source_key("/projects/rune", "project");
    click(&mut rt, &project);
    click(
        &mut rt,
        &crate::code_view::source_key("/projects/caelum", "project"),
    );
    let close = crate::code_view::source_key("/projects/rune", "close-project");
    click(&mut rt, &close);
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    let first = crate::code_view::source_key("/projects/caelum", "project");
    click(&mut rt, &first);
    key(&mut rt, Key::Tab, Mods::default());
    key(&mut rt, Key::Right, Mods::default());
    advance(&mut rt, 0.2);
    assert!(
        element(
            &rt,
            &crate::code_view::source_key("/projects/caelum", "close-project")
        )
        .focused,
        "focused: {:?}",
        rt.elements()
            .iter()
            .filter(|e| e.focused)
            .map(|e| &e.key)
            .collect::<Vec<_>>()
    );
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    advance(&mut rt, 0.4);
    assert!(
        !rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .project_is_open("/projects/caelum")
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Retain this project draft café"
    );
    click(&mut rt, "search-sessions");
    fill(&mut rt, "session-search", "Refine");
    assert!(rt.find("live-session-0").is_none());
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.open_project("/projects/caelum".into());
        let open = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Open(_)))
            .unwrap()
            .0;
        l.apply(Event::Completed(
            open,
            Ok(Reply::Project(caelum_protocol::ProjectInfo {
                path: "/projects/caelum".into(),
                name: "caelum".into(),
            })),
        ));
    });
    advance(&mut rt, 0.4);
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Retain this project draft café"
    );
    for path in ["/projects/website", "/projects/tools", "/projects/caelum"] {
        click(
            &mut rt,
            &crate::code_view::source_key(path, "close-project"),
        );
        advance(&mut rt, 0.4);
    }
    assert!(rt.state.live.as_ref().unwrap().app.selected.is_none());
    assert!(element(&rt, "open-project").focused);
    assert!(rt.find("welcome-open").is_some());
    assert!(rt.find("message-composer").is_none());
}

#[test]
fn narrow_composer_wraps_all_options_and_keeps_queue_and_stop_reachable() {
    for (size, inspector) in [
        ((800.0, 620.0), false),
        ((800.0, 620.0), true),
        ((1280.0, 1000.0), false),
        ((1280.0, 1000.0), true),
    ] {
        let mut rt = design_runtime("composer-options", size);
        rt.update(|s, _| s.inspector.set_open(inspector));
        for working in [false, true] {
            rt.update(|s, _| {
                s.live.as_mut().unwrap().app.sessions[0]
                    .conversation
                    .working = working
            });
            advance(&mut rt, 0.3);
            for name in ["model-trigger", "effort-trigger", "agent-trigger", "send"] {
                let e = element(&rt, name);
                assert!(e.rect[2] >= 32.0 && e.rect[3] >= 28.0, "{name}: {e:?}");
                assert!(
                    e.rect
                        .iter()
                        .zip(e.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{name} clipped: {e:?}"
                );
                assert!(e.rect[1] + e.rect[3] <= size.1, "{name} below window");
            }
            assert!(element(&rt, "composer-editor").visible[3] >= 46.0);
            let composer = element(&rt, "message-composer").rect;
            let send = element(&rt, "send").rect;
            for axis in [0, 1] {
                let inset = composer[axis] + composer[axis + 2] - send[axis] - send[axis + 2];
                assert!(
                    (inset - 12.0).abs() < 0.5,
                    "Send/Stop inset: {inset}; size={size:?}, inspector={inspector}, working={working}"
                );
            }
            if working {
                assert!(element(&rt, "queue-message").visible[2] >= 50.0);
                assert_eq!(
                    element(&rt, "send").label.as_deref(),
                    Some("Stop generation")
                );
            }
            assert!(rt.find("provider-trigger").is_none());
            for name in ["effort", "agent"] {
                let trigger = format!("{name}-trigger");
                click(&mut rt, &trigger);
                let menu = element(&rt, &format!("{name}-content"));
                assert!(
                    menu.rect
                        .iter()
                        .zip(menu.visible)
                        .all(|(a, b)| (*a - b).abs() < 1.0),
                    "{menu:?}"
                );
                key(&mut rt, Key::Escape, Mods::default());
                advance(&mut rt, 0.2);
                assert!(element(&rt, &trigger).focused);
            }
            assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
        }
    }
}

#[test]
fn composer_menus_use_provider_ids_restore_defaults_and_dismiss_on_session_change() {
    let mut rt = design_runtime("composer-options", (800.0, 620.0));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        let scope = app.sessions[0].scope.clone();
        app.choices
            .get_mut(&scope)
            .unwrap()
            .agents
            .push(caelum_protocol::ProviderChoice {
                id: "custom-review".into(),
                name: "Review assistant".into(),
            });
    });
    advance(&mut rt, 0.2);
    let draft = rt.state.live.as_ref().unwrap().drafts[0].text.clone();
    click(&mut rt, "agent-trigger");
    assert_eq!(
        element(&rt, "option3").label.as_deref(),
        Some("Review assistant")
    );
    click(&mut rt, "option3");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .options
            .agent
            .as_deref(),
        Some("custom-review")
    );
    click(&mut rt, "effort-trigger");
    key(&mut rt, Key::End, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .options
            .effort
            .as_deref(),
        Some("Low")
    );
    for name in ["effort", "agent"] {
        click(&mut rt, &format!("{name}-trigger"));
        click(&mut rt, "option0");
    }
    let options = &rt.state.live.as_ref().unwrap().app.sessions[0].options;
    assert!(options.effort.is_none() && options.agent.is_none());
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, draft);
    let other = rt.state.live.as_ref().unwrap().app.sessions[1]
        .options
        .clone();
    click(&mut rt, "agent-trigger");
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
    advance(&mut rt, 0.3);
    assert!(rt.find("agent-content").is_none());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[1].options,
        other
    );
}

#[test]
fn file_tabs_reveal_keyboard_selection_and_restore_focus_after_closing() {
    let mut rt = design_runtime("files-tabs", (800.0, 620.0));
    let project = "/projects/caelum";
    let paths = [
        "Cargo.toml",
        "src/application/independent_session_workspace.rs",
        "README.md",
    ];
    let tabs =
        paths.map(|path| format!("file-tab-{}", crate::code_view::source_key(project, path)));
    click(&mut rt, &tabs[0]);
    for i in [1, 2] {
        key(&mut rt, Key::Right, Mods::default());
        advance(&mut rt, 0.4);
        assert!(element(&rt, &tabs[i]).focused);
        assert_eq!(
            rt.state.live.as_ref().unwrap().app.services.checkouts[project]
                .selected_file
                .as_deref(),
            Some(paths[i])
        );
        let e = element(&rt, &tabs[i]);
        assert!(
            (e.visible[2] - e.rect[2]).abs() < 1.0,
            "selected tab clipped: {e:?}"
        );
    }
    key(&mut rt, Key::Tab, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, &format!("close-{}", tabs[2])).focused);
    key(&mut rt, Key::Text(" ".into()), Mods::default());
    advance(&mut rt, 0.4);
    assert!(element(&rt, &tabs[1]).focused);
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.services.checkouts[project]
            .tabs
            .len(),
        2
    );
    click(&mut rt, "attach-preview");
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
    advance(&mut rt, 0.2);
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0].attachments[0].path,
        paths[1]
    );
    assert!(
        rt.state.live.as_ref().unwrap().app.sessions[1]
            .attachments
            .is_empty()
    );
    for i in [1, 0] {
        let close = format!("close-{}", tabs[i]);
        let (x, y) = rt.find(&close).unwrap();
        rt.pointer_move(x, y);
        advance(&mut rt, 0.2);
        click(&mut rt, &close);
        advance(&mut rt, 0.3);
    }
    assert!(element(&rt, "find-files").focused);
    assert!(
        rt.state.live.as_ref().unwrap().app.services.checkouts[project]
            .selected_file
            .is_none()
    );
}

#[test]
fn merge_states_expose_only_available_actions_in_the_compact_overview() {
    use caelum_core::application::{Command, Task};
    for scene in [
        "merge-ready",
        "merge-behind",
        "merge-blocked",
        "merge-pending",
        "merge-conflicts",
        "merge-draft",
        "merge-merged",
        "merge-closed",
    ] {
        let mut rt = design_runtime(scene, (800.0, 620.0));
        let present = !matches!(scene, "merge-draft" | "merge-merged" | "merge-closed");
        assert_eq!(rt.find("merge-pull-request").is_some(), present, "{scene}");
        if present {
            let e = element(&rt, "merge-pull-request");
            assert!(e.visible[3] >= 33.0, "{scene} merge clipped: {e:?}");
            let available = matches!(scene, "merge-ready" | "merge-behind");
            assert_eq!(e.focusable, available, "{scene}");
            if !available {
                click(&mut rt, "merge-pull-request");
                assert!(
                    !rt.state
                        .live
                        .as_mut()
                        .unwrap()
                        .app
                        .commands()
                        .iter()
                        .any(|(_, c)| matches!(c, Command::Task(Task::Merge { .. })))
                );
            }
        }
    }
}

#[test]
fn merge_checks_methods_and_mutations_retain_the_review_owner() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    use caelum_protocol::GitHubMergeMethod;
    for update in [false, true] {
        let mut rt = design_runtime(
            if update {
                "merge-behind"
            } else {
                "merge-ready"
            },
            (800.0, 620.0),
        );
        click(&mut rt, "review-checks-toggle");
        assert_eq!(element(&rt, "review-checks-toggle").expanded, Some(true));
        assert!(element(&rt, "check-name-0").visible[2] > 100.0);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(element(&rt, "review-checks-toggle").expanded, Some(false));
        click(&mut rt, "merge-method-trigger");
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.2);
        assert!(element(&rt, "merge-method-trigger").focused);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        advance(&mut rt, 0.2);
        key(&mut rt, Key::End, Mods::default());
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(
            element(&rt, "merge-pull-request").label.as_deref(),
            Some("Rebase and merge")
        );
        let action = if update {
            "update-review-branch"
        } else {
            "merge-pull-request"
        };
        click(&mut rt, action);
        let (request, command) = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| {
                matches!(
                    c,
                    Command::Task(Task::Merge { .. } | Task::UpdateBranch { .. })
                )
            })
            .unwrap();
        let original = (
            "/projects/caelum".to_string(),
            "https://github.com/ch55secake/caelum/pull/95".to_string(),
        );
        match command {
            Command::Task(Task::UpdateBranch { project, url, head }) if update => {
                assert_eq!((project, url), original);
                assert_eq!(head, "a".repeat(40));
            }
            Command::Task(Task::Merge {
                project,
                url,
                head,
                method,
            }) if !update => {
                assert_eq!((project, url), original);
                assert_eq!(head, "a".repeat(40));
                assert_eq!(method, GitHubMergeMethod::Rebase);
            }
            _ => panic!("wrong mutation"),
        }
        assert!(!element(&rt, "merge-method-trigger").focusable);
        click(&mut rt, action);
        assert!(
            !rt.state
                .live
                .as_mut()
                .unwrap()
                .app
                .commands()
                .iter()
                .any(|(_, c)| matches!(
                    c,
                    Command::Task(Task::Merge { .. } | Task::UpdateBranch { .. })
                ))
        );
        rt.update(|s, _| {
            crate::panels::open_review(
                s,
                "/other".into(),
                "https://github.com/example/other/pull/2".into(),
            );
            let l = s.live.as_mut().unwrap();
            l.app.select(1);
            l.app.commands();
            l.apply(Event::Completed(request, Ok(Reply::Task(TaskReply::Done))));
        });
        advance(&mut rt, 0.2);
        let l = rt.state.live.as_mut().unwrap();
        assert_eq!(l.app.selected, Some(1));
        assert_eq!(l.panels.review.as_ref().unwrap().0, "/other");
        assert!(l.app.services.reviews[&original].notice.is_some());
        assert!(l.app.commands().iter().any(|(_, c)| matches!(c, Command::Task(Task::Details { project, url }) if project == &original.0 && url == &original.1)));
    }
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

#[test]
fn background_previews_are_bounded_and_output_actions_keep_their_session() {
    use caelum_core::application::{Command, Task};
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("shells-many", size);
        assert!(element(&rt, "shell-preview").rect[3] <= 50.1);
        assert!(element(&rt, "background-shells").rect[3] <= 140.1);
        assert!(element(&rt, "composer-editor").visible[3] >= 46.0);
        click(&mut rt, "toggle-background-shells");
        advance(&mut rt, 0.3);
        let collapsed = element(&rt, "background-shells");
        assert!(collapsed.rect[3] <= 24.1, "{collapsed:?}");
        assert!(rt.find("shell-preview").is_none());
        assert!(rt.find("view-shell-output").is_none());
        assert!(element(&rt, "toggle-background-shells").focused);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        advance(&mut rt, 0.2);
        assert!(element(&rt, "toggle-background-shells").focused);
        assert_eq!(
            element(&rt, "toggle-background-shells").expanded,
            Some(true)
        );
        click(&mut rt, "view-shell-output");
        advance(&mut rt, 0.3);
        let output = element(&rt, "shell-output-shell_fixture");
        assert!(output.visible[3] >= 80.0 && output.rect[3] < size.1);
        assert!(
            rt.elements()
                .iter()
                .filter_map(|e| e.text.as_deref())
                .all(|text| !text.contains("shell_fixture") && !text.contains("shell-1"))
        );
        rt.update(|s, _| s.live.as_mut().unwrap().app.select(1));
        advance(&mut rt, 0.2);
        click(&mut rt, "output-stop-shell-shell_fixture");
        let commands = rt.state.live.as_mut().unwrap().app.commands();
        assert!(commands.iter().any(
            |(_, c)| matches!(c, Command::Task(Task::Shell { project, session, id, stop:true })
            if project == "/projects/caelum" && session == "ses_fixture" && id == "shell_fixture")
        ));
    }
}

#[test]
fn collapsed_background_commands_keep_running_and_remain_session_local() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = design_runtime("shells-many", (800.0, 620.0));
    fill(
        &mut rt,
        "live-draft-0",
        "Keep this draft while commands run",
    );
    click(&mut rt, "toggle-background-shells");
    let read = Task::Shell {
        project: "/projects/caelum".into(),
        session: "ses_fixture".into(),
        id: "shell_fixture".into(),
        stop: false,
    };
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        assert!(!l.app.commands().iter().any(|(_, c)| matches!(
            c,
            Command::Cancel(_) | Command::Task(Task::Shell { stop: true, .. })
        )));
        l.app.task(read.clone());
        let request = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| {
                matches!(c,
            Command::Task(Task::Shell { id, stop: false, .. }) if id == "shell_fixture")
            })
            .unwrap()
            .0;
        l.app.select(1);
        l.app.sessions[1].scope.provider = caelum_core::application::Provider::OpenCode;
        l.app.sessions[1].remote_id = Some("ses_other".into());
        l.app.sessions[1].shells = l.app.sessions[0].shells.clone();
        l.apply(Event::Completed(
            request,
            Ok(Reply::Task(TaskReply::Shell(
                "Fresh output while hidden".into(),
            ))),
        ));
    });
    advance(&mut rt, 0.3);
    assert_eq!(
        element(&rt, "toggle-background-shells").expanded,
        Some(true)
    );
    assert!(rt.find("shell-preview").is_some());
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(0));
    advance(&mut rt, 0.3);
    assert_eq!(
        element(&rt, "toggle-background-shells").expanded,
        Some(false)
    );
    assert!(rt.find("shell-preview").is_none());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0].shells.len(),
        8
    );
    click(&mut rt, "toggle-background-shells");
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Fresh output while hidden"))
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep this draft while commands run"
    );
}

#[test]
fn vercel_theme_is_keyboard_selectable_and_retains_mode_and_accent_on_restore() {
    use caelum_core::application::{Event, Reply, Snapshot};
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("vercel-settings", size);
        click(&mut rt, "accent-violet");
        click(&mut rt, "family-caelum");
        key(&mut rt, Key::Tab, Mods::default());
        advance(&mut rt, 0.1);
        assert!(element(&rt, "family-vercel").focused);
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        advance(&mut rt, 0.2);
        assert_eq!(
            rt.state.live.as_ref().unwrap().app.preferences.accent_color,
            "theme"
        );
        for (family, _) in crate::theme::FAMILIES {
            let card = element(&rt, &format!("family-{family}"));
            assert!(
                card.rect
                    .iter()
                    .zip(card.visible)
                    .all(|(a, b)| (*a - b).abs() < 1.0),
                "{card:?}"
            );
        }
        click(&mut rt, "accent-teal");
        click(&mut rt, "family-vercel");
        for mode in ["light", "dark", "system"] {
            click(&mut rt, &format!("theme-{mode}"));
            advance(&mut rt, 0.3);
            let p = &rt.state.live.as_ref().unwrap().app.preferences;
            assert_eq!(p.theme, mode);
            assert_eq!(p.theme_family, "vercel");
            assert_eq!(p.accent_color, "teal");
        }
        let snapshot: Snapshot = serde_json::from_slice(
            &serde_json::to_vec(&rt.state.live.as_ref().unwrap().app.snapshot()).unwrap(),
        )
        .unwrap();
        let mut restored = caelum_core::application::Workspace::default();
        restored.enable_store("unused-fixture-state.json".into());
        let load = restored.commands().pop().unwrap().0;
        restored.apply(Event::Completed(
            load,
            Ok(Reply::Snapshot(Some(Box::new(snapshot)))),
        ));
        assert_eq!(restored.preferences.theme_family, "vercel");
        assert_eq!(restored.preferences.accent_color, "teal");
    }
    let luminance = |c: rune::Color| {
        let linear = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    };
    for dark in [false, true] {
        for accent in crate::theme::ACCENTS {
            let k = crate::theme::appearance(dark, "vercel", accent);
            let foreground = luminance(k.c.submit_fg);
            let background = luminance(k.c.bg_icon_button_contrast);
            assert!(
                (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05) >= 4.5,
                "unreadable Vercel submit control: {dark}, {accent}"
            );
        }
    }
}

#[test]
fn user_bubbles_size_to_content_group_images_and_copy_the_original_markdown() {
    let mut rt = design_runtime("messages", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let bubbles: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| {
            e.key
                .as_ref()
                .is_some_and(|k| k.starts_with("user-bubble-"))
                && !e.exiting
        })
        .collect();
    assert_eq!(bubbles.len(), 3);
    let short = bubbles
        .iter()
        .find(|b| b.key.as_ref().unwrap().contains("short"))
        .unwrap();
    assert!(short.rect[2] < 260.0);
    assert!(
        bubbles
            .iter()
            .all(|b| (b.rect[0] + b.rect[2] - (short.rect[0] + short.rect[2])).abs() < 1.0)
    );
    let formatted = rt
        .elements()
        .into_iter()
        .find(|e| {
            e.key
                .as_ref()
                .is_some_and(|k| k.starts_with("copy-user-") && k.contains("formatted"))
        })
        .unwrap()
        .key
        .unwrap();
    let (x, y) = rt.find(&formatted).unwrap();
    rt.pointer_move(x, y);
    advance(&mut rt, 0.2);
    click(&mut rt, &formatted);
    assert!(clipboard.text().unwrap().contains("**session owner**"));
    assert!(clipboard.text().unwrap().contains("```rust"));
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.role == "img" && e.path.contains("user-bubble-"))
    );
    let file = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("src/session.rs"))
        .expect("grouped file label");
    assert!(
        file.path.contains("user-bubble-") && file.visible[2] > 60.0,
        "{file:?}"
    );
}

#[test]
fn review_overview_has_a_readable_column_aligned_checks_and_conditional_feedback() {
    let mut rt = design_runtime("overview-empty", (1600.0, 1000.0));
    let column = element(&rt, "review-overview-scroll");
    assert!(column.rect[2] <= 960.1);
    assert!(
        !rt.elements()
            .iter()
            .any(|e| !e.exiting && e.key.as_deref() == Some("address-comments"))
    );
    click(&mut rt, "merge-method-trigger");
    key(&mut rt, Key::Down, Mods::default());
    key(&mut rt, Key::Down, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.2);
    assert!(
        !rt.elements()
            .iter()
            .any(|e| !e.exiting && e.key.as_deref() == Some("merge-method-content"))
    );
    let mut rt = design_runtime("checks", (1280.0, 1000.0));
    click(&mut rt, "review-checks-toggle");
    let first = element(&rt, "check-name-0");
    let second = element(&rt, "check-name-1");
    assert!((first.rect[0] - second.rect[0]).abs() < 1.0);
    assert!(element(&rt, "address-comments").visible[3] >= 30.0);
}

#[test]
fn native_shortcuts_and_large_model_catalogue_keep_the_captured_owner() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let scope = l.app.sessions[1].scope.clone();
        l.app.choices.insert(scope, serde_json::from_value(serde_json::json!({
            "models": (0..160).map(|i| serde_json::json!({"id":format!("model-{i:03}"),"name":format!("Model {i:03}"),"efforts":[],"defaultEffort":null})).collect::<Vec<_>>(),
            "agents":[{"id":"build","name":"Build"},{"id":"plan","name":"Plan"}],"defaultModel":"model-000"
        })).unwrap());
        l.app.sessions[1].options.model = Some("model-000".into());
        l.app.sessions[1].options.agent = Some("build".into());
        l.app.select(1);
    });
    advance(&mut rt, 0.3);
    fill(&mut rt, "live-draft-1", "Preserve café and the caret");
    key(
        &mut rt,
        Key::Tab,
        Mods {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[1]
            .options
            .agent
            .as_deref(),
        Some("plan")
    );
    assert!(element(&rt, "live-draft-1").focused);
    click(&mut rt, "model-trigger");
    advance(&mut rt, 0.3);
    for _ in 0..105 {
        key(&mut rt, Key::Down, Mods::default());
    }
    advance(&mut rt, 0.2);
    assert!(element(&rt, "option106").visible[3] > 0.0);
    assert!(rt.elements().len() < 1000, "catalogue must be windowed");
    key(&mut rt, Key::Tab, Mods::default());
    assert!(!element(&rt, "model-search").focused);
    key(&mut rt, Key::Down, Mods::default());
    advance(&mut rt, 0.2);
    assert!(element(&rt, "model-search").focused);
    assert!(element(&rt, "option107").visible[3] > 0.0);
    fill(&mut rt, "model-search", "159");
    rt.update(|s, _| s.live.as_mut().unwrap().app.select(0));
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.3);
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.app.selected, Some(0));
    assert_eq!(
        l.app.sessions[1].options.model.as_deref(),
        Some("model-159")
    );
    assert_eq!(l.drafts[1].text, "Preserve café and the caret");
    assert_ne!(
        l.app.sessions[0].options.model.as_deref(),
        Some("model-159")
    );
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.sessions[0].remote_id = Some("thread".into());
        l.app.sessions[0].conversation.working = true;
    });
    key(
        &mut rt,
        Key::Command(','),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.3);
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.3);
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
}

#[test]
fn tool_patches_share_the_source_renderer_for_fragments_and_multiple_files() {
    let files = crate::code_view::tool_files("file.rs", "@@ -1 +1 @@\n-old();\n+new();");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path(), "file.rs");
    assert_eq!(files[0].hunks[0].lines.len(), 2);
    let at = crate::code_view::tool_files("file.rs", "@@ -40 +40 @@\n-old();\n+new();");
    assert_eq!(
        at[0].hunks[0].lines[1].number(rune_kit::diff::Side::New),
        Some(40)
    );
    let files = crate::code_view::tool_files(
        "Patch",
        "*** Begin Patch\n*** Update File: src/a.rs\n@@\n-old\n+new\n*** Add File: src/b.rs\n+created\n*** End Patch",
    );
    assert_eq!(
        files.iter().map(|f| f.path()).collect::<Vec<_>>(),
        ["src/a.rs", "src/b.rs"]
    );
}

#[test]
fn model_popover_stays_anchored_and_returns_focus_without_cancelling_work() {
    let app = spec();
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, "models");
    let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    advance(&mut rt, 0.4);
    let popup = element(&rt, "model-popover");
    let trigger = element(&rt, "model-trigger");
    assert!(popup.rect[1] + popup.rect[3] <= trigger.rect[1]);
    assert!(
        popup
            .rect
            .into_iter()
            .zip(popup.visible)
            .all(|(a, b)| (a - b).abs() < 1.0)
    );
    assert!(
        !rt.elements()
            .iter()
            .any(|e| !e.exiting && e.key.as_deref() == Some("workspace-dialog"))
    );
    let defaults = element(&rt, "save-provider-defaults");
    assert!(defaults.rect[1] + defaults.rect[3] <= popup.rect[1] + popup.rect[3]);
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .working = true;
    });
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.4);
    assert!(!rt.state.live.as_ref().unwrap().models.is_open());
    assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    click(&mut rt, "model-trigger");
    rt.pointer_down(250.0, 80.0);
    rt.pointer_up(250.0, 80.0);
    advance(&mut rt, 0.4);
    assert!(!rt.state.live.as_ref().unwrap().models.is_open());
}

#[test]
fn models_slash_command_opens_the_picker_for_the_current_session() {
    let mut rt = live_runtime_size((800.0, 620.0));
    fill(&mut rt, "live-draft-0", "/models");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.3);

    let live = rt.state.live.as_ref().unwrap();
    assert!(live.models.is_open());
    assert!(live.app.sessions[0].draft.is_empty());
    assert!(!live.app.sessions[0].submitting);
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    assert!(commands.iter().any(|(_, command)| {
        matches!(
            command,
            caelum_core::application::Command::Choices(scope)
                if scope.project == "/project"
                    && scope.provider == caelum_core::application::Provider::Codex
        )
    }));
    assert!(
        rt.elements()
            .iter()
            .any(|element| { !element.exiting && element.key.as_deref() == Some("model-popover") })
    );
}

#[test]
fn reminders_are_absent_from_transcript_copy_and_timeline_with_stable_jumps() {
    let app = spec();
    let mut state = crate::evaluation::design();
    let l = state.live.as_mut().unwrap();
    l.app.sessions[0].conversation = Default::default();
    let mut messages = vec![
        serde_json::json!({"id":"prompt","type":"user","text":"Visible café prompt"}),
        serde_json::json!({"id":"answer","type":"assistant","content":[{"id":"text","type":"text","text":"Visible answer<system-reminder>internal secret</system-reminder>"}]}),
    ];
    messages.extend((0..30).map(|i| serde_json::json!({"id":format!("hidden-{i}"),"type":"synthetic","text":"<system-reminder>internal secret</system-reminder>"})));
    l.app.sessions[0].conversation.history(
        caelum_core::application::Provider::OpenCode,
        &messages,
        false,
    );
    let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    advance(&mut rt, 0.5);
    assert!(!rt.elements().iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.contains("internal secret"))
    }));
    assert!(element(&rt, "live-entry-0-opencode-prompt").visible[3] > 20.0);
    click(&mut rt, "copy-response-0-opencode-answer");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some("Visible answer")
    );
    fill(&mut rt, "live-draft-0", "/timeline");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.3);
    fill(&mut rt, "timeline-search", "internal secret");
    assert!(!rt.elements().iter().any(|e| {
        !e.exiting
            && e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("timeline-opencode"))
    }));
    fill(&mut rt, "timeline-search", "café");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .history(
                caelum_core::application::Provider::OpenCode,
                &[serde_json::json!({"id":"older","type":"user","text":"Earlier history"})],
                true,
            )
    });
    key(&mut rt, Key::Down, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.5);
    assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
    assert!(element(&rt, "live-entry-0-opencode-prompt").visible[3] > 20.0);
}

#[test]
fn composer_activity_is_visible_without_sidebar_and_settles_with_reduced_motion() {
    for reduced in [false, true] {
        let mut rt = live_runtime_size((800.0, 620.0));
        rt.reduced_motion = reduced;
        fill(&mut rt, "live-draft-0", "Start working");
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.3);
        assert!(element(&rt, "session-activity").visible[3] > 15.0);
        rt.update(|s, _| {
            s.sidebar.set_open(false);
        });
        advance(&mut rt, 0.4);
        assert!(element(&rt, "composer-pulse").visible[2] > 200.0);
        rt.update(|s, _| {
            let session = &mut s.live.as_mut().unwrap().app.sessions[0];
            session.submitting = false;
            session.conversation.settle();
        });
        advance(&mut rt, 0.4);
        assert!(
            !rt.elements()
                .iter()
                .any(|e| !e.exiting && e.key.as_deref() == Some("session-activity"))
        );
    }
}

#[test]
fn default_patch_previews_are_lazy_and_explicit_collapse_survives_stream_updates() {
    let app = spec();
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, "tools");
    state.live.as_mut().unwrap().expanded.clear();
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    advance(&mut rt, 0.5);
    let l = rt.state.live.as_ref().unwrap();
    let id = l.app.sessions[0]
        .conversation
        .entries
        .iter()
        .find(|e| e.name == "apply_patch")
        .unwrap()
        .id
        .clone();
    assert!(l.patches.contains_key(&(0, id.clone())));
    assert!(rt.elements().iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.contains("restore_owned_draft"))
    }));
    click(&mut rt, &format!("tool-0-{id}"));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let entry = l.app.sessions[0]
            .conversation
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap();
        entry.output.push_str("\nAnother output event");
    });
    advance(&mut rt, 0.5);
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .collapsed_patches
            .contains(&(0, id.clone()))
    );
    assert!(!rt.elements().iter().any(|e| {
        !e.exiting
            && e.text
                .as_deref()
                .is_some_and(|t| t.contains("restore_owned_draft"))
    }));
}

#[test]
fn compact_review_keeps_code_and_the_targeted_feedback_action_visible() {
    let app = spec();
    let mut state = crate::evaluation::design();
    crate::evaluation::scene(&mut state, "review");
    let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    advance(&mut rt, 0.5);
    let diff = rt
        .elements()
        .into_iter()
        .find(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
        .unwrap();
    assert!(
        diff.visible[3] >= 170.0,
        "compact code viewport: {:?}",
        diff.visible
    );
    let action = element(&rt, "address-comments");
    assert!(action.visible[3] >= 30.0);
    click(&mut rt, "review-nav-toggle");
    advance(&mut rt, 0.3);
    fill(&mut rt, "review-file-search", "composer");
    click(&mut rt, "review-file-src/composer.rs");
    click(&mut rt, "review-nav-toggle");
    advance(&mut rt, 0.3);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref().is_some_and(|t| t.contains("MAX_HEIGHT")))
    );
}

#[test]
fn settings_dropdown_escape_dismisses_only_the_foremost_surface() {
    for dark in [false, true] {
        let app = spec();
        let mut state = crate::evaluation::design();
        crate::evaluation::scene(&mut state, "providers");
        state.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .working = true;
        let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(dark, 1.0));
        rt.poll = app.poll;
        rt.on_key = app.on_key;
        advance(&mut rt, 0.4);
        click(&mut rt, "Default effort-trigger");
        advance(&mut rt, 0.3);
        assert!(element(&rt, "option1").visible[3] > 20.0);
        click(&mut rt, "option1");
        assert_eq!(
            rt.state.live.as_ref().unwrap().app.preferences.providers["codex"]
                .effort
                .as_deref(),
            Some("Medium")
        );
        click(&mut rt, "Default effort-trigger");
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .panels
                .settings_menu
                .is_none()
        );
        assert_eq!(
            rt.state.live.as_ref().unwrap().panels.overlay,
            Some(crate::panels::Screen::Settings)
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.3);
        assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
        assert!(!rt.state.live.as_ref().unwrap().app.sessions[0].stopping);
    }
}

#[test]
fn long_drafts_and_crowded_sidebar_keep_controls_in_bounds() {
    let mut rt = live_runtime_size((800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        for n in 0..35 {
            let i = l.app.new_session(caelum_core::application::Scope {
                project: "/project".into(),
                provider: caelum_core::application::Provider::Codex,
            });
            l.app.sessions[i].title = format!("Session {n}");
        }
        l.app.select(0);
        l.poll(&mut rune::Tx::default());
        l.app.archive(2, true);
    });
    advance(&mut rt, 0.3);
    fill(
        &mut rt,
        "live-draft-0",
        &"A long Unicode draft café ☁️\n".repeat(30),
    );
    for name in ["nav-settings", "undo-archive", "send", "model-trigger"] {
        let e = element(&rt, name);
        assert!(
            e.visible[2] > 0.0 && e.visible[3] > 0.0,
            "{name} is clipped"
        );
        assert!(
            e.rect[1] >= 0.0 && e.rect[1] + e.rect[3] <= 620.1,
            "{name} outside the window: {:?}",
            e.rect
        );
    }
    let editor = element(&rt, "composer-editor");
    let send = element(&rt, "send");
    assert!(editor.rect[3] <= 180.1);
    assert!(editor.rect[1] + editor.rect[3] <= send.rect[1] + 1.0);
    assert!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .ends_with("café ☁️\n")
    );
    click(&mut rt, "nav-settings");
    advance(&mut rt, 0.2);
    for page in [
        "providers",
        "worktrees",
        "notifications",
        "about",
        "appearance",
    ] {
        click(&mut rt, &format!("settings-{page}"));
        assert!(element(&rt, &format!("settings-{page}")).visible[3] > 0.0);
    }
    key(&mut rt, Key::Escape, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0]
            .text
            .lines()
            .count(),
        30
    );
}

#[test]
fn native_mentions_replace_only_the_caret_token_and_escape_cancels_pending_reads() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Before café @ma after");
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let at = l.drafts[0].text.find(" after").unwrap();
        l.drafts[0].set_caret(at);
    });
    advance(&mut rt, 0.1);
    let requests = rt.state.live.as_mut().unwrap().app.commands();
    let (search, _) = requests
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Completions { .. })))
        .unwrap();
    let results=serde_json::from_value(serde_json::json!({"query":"ma","matches":[{"path":"main.rs","line":null,"preview":null}],"truncated":false,"skipped":0})).unwrap();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            search,
            Ok(Reply::Task(TaskReply::Search(results))),
        ))
    });
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Tab, Mods::default());
    let (read, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    key(&mut rt, Key::Escape, Mods::default());
    assert_eq!(rt.state.live.as_ref().unwrap().app.sessions[0].attaching, 0);
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            read,
            Ok(Reply::Task(TaskReply::File(
                caelum_protocol::ProjectFileContent {
                    path: "main.rs".into(),
                    content: "snapshot".into(),
                    size_bytes: 8,
                    truncated: false,
                },
            ))),
        ))
    });
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Before café @ma after"
    );
    assert!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .attachments
            .is_empty()
    );
}

#[test]
fn native_side_questions_preserve_main_draft_focus_and_reject_late_answers() {
    use caelum_core::application::{Command, Event, Reply};
    let mut rt = live_runtime();
    click(&mut rt, "live-session-1");
    rt.update(|s, _| {
        let session = &mut s.live.as_mut().unwrap().app.sessions[1];
        session.remote_id = Some("existing".into());
        session.loaded = true;
        session.conversation.working = true;
    });
    fill(&mut rt, "live-draft-1", "Keep the main draft");
    fill(&mut rt, "live-draft-1", "/btw Why does this work?");
    key(&mut rt, Key::Enter, Mods::default());
    assert!(rt.find("side-question").is_some());
    let commands = rt.state.live.as_mut().unwrap().app.commands();
    let (request, _) = commands
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Generate(..)))
        .unwrap();
    key(&mut rt, Key::Escape, Mods::default());
    advance(&mut rt, 0.2);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "Keep the main draft"
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().apply(Event::Completed(
            request,
            Ok(Reply::Generated("Late answer".into())),
        ))
    });
    assert!(rt.find("side-question").is_none());
    assert!(rt.state.live.as_ref().unwrap().app.sessions[1].working());
    rt.paste(" plus more");
    advance(&mut rt, 0.1);
    assert!(
        rt.state.live.as_ref().unwrap().drafts[1]
            .text
            .contains("plus more")
    );
}

#[test]
fn source_search_navigation_and_refresh_preserve_the_reading_position() {
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let c = l
            .app
            .services
            .checkouts
            .entry("/project".into())
            .or_default();
        for path in ["a.rs", "b.rs"] {
            c.tabs.push(path.into());
            c.files.insert(
                path.into(),
                caelum_protocol::ProjectFileContent {
                    path: path.into(),
                    content: (1..1000)
                        .map(|n| format!("let café_{n} = {n};\n"))
                        .collect(),
                    size_bytes: 20000,
                    truncated: false,
                },
            );
        }
        c.selected_file = Some("a.rs".into());
        l.app.services.revision += 1;
        super::panels::show(s, super::panels::Screen::Files);
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "find-in-file");
    fill(&mut rt, "local-file-search", "café_500");
    advance(&mut rt, 0.5);
    assert!(
        rt.elements().iter().any(
            |e| e.text.as_deref().is_some_and(|t| t.contains("café_500")) && e.visible[3] > 0.0
        )
    );
    let saved = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .snapshot()
        .preferences
        .files["/project"]
        .scroll["a.rs"];
    assert!(
        saved[1] > 1000.0,
        "actual code scrolling is persisted: {saved:?}"
    );
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.open_file("/project".into(), "b.rs".into());
    });
    advance(&mut rt, 0.2);
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        l.app.open_file("/project".into(), "a.rs".into());
    });
    advance(&mut rt, 0.3);
    let restored = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .snapshot()
        .preferences
        .files["/project"]
        .scroll["a.rs"];
    assert!(
        (saved[1] - restored[1]).abs() < 1.0,
        "file switching must preserve its pixel offset: {saved:?}, {restored:?}"
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.services.revision += 1;
    });
    advance(&mut rt, 0.2);
    assert_eq!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .snapshot()
            .preferences
            .files["/project"]
            .scroll["a.rs"],
        restored
    );
}

#[test]
fn editing_sending_background_streaming_and_stopping_stay_session_scoped() {
    let mut rt = runtime((1280.0, 1000.0));
    let original = rt.state.active().draft.text.clone();
    click(&mut rt, "session-1");
    click(&mut rt, "draft-1");
    rt.paste("Review café ☁️");
    key(
        &mut rt,
        Key::Enter,
        Mods {
            shift: true,
            ..Mods::default()
        },
    );
    rt.paste("Keep the draft isolated.");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns[0].prompt,
        "Review café ☁️\nKeep the draft isolated."
    );
    assert!(rt.state.active().working());
    assert!(rt.state.active().draft.text.is_empty());

    rt.paste("Next message");
    key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.active().turns.len(),
        1,
        "no second send during a stream"
    );
    click(&mut rt, "session-0");
    advance(&mut rt, 6.0);
    assert_eq!(rt.state.active().draft.text, original);
    assert!(!rt.state.sessions[1].working());
    assert!(
        rt.state.sessions[1].turns[0]
            .response
            .ends_with("sample patch.")
    );

    click(&mut rt, "session-1");
    assert_eq!(rt.state.active().draft.text, "Next message");
    click(&mut rt, "send");
    advance(&mut rt, 0.5);
    click(&mut rt, "send");
    let stopped = rt.state.active().turns[1].response.clone();
    assert!(!stopped.is_empty());
    assert!(rt.state.active().turns[1].stopped);
    advance(&mut rt, 6.0);
    assert_eq!(
        rt.state.active().turns[1].response,
        stopped,
        "stop cancels future chunks"
    );
    click(&mut rt, "draft-1");
    rt.paste("Try the keyboard stop shortcut");
    key(&mut rt, Key::Enter, Mods::default());
    advance(&mut rt, 0.2);
    key(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.active().working());
    assert!(rt.state.active().turns[2].stopped);
}

#[test]
fn panes_theme_tools_and_diff_preserve_the_conversation_and_draft() {
    let mut rt = runtime((1280.0, 1000.0));
    let draft = rt.state.active().draft.text.clone();
    let response = rt.state.active().turns[0].response.clone();
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "copy-0");
    assert_eq!(
        clipboard.text().as_deref().map(String::as_str),
        Some(response.as_str())
    );
    click(&mut rt, "tool-0");
    assert!(rt.state.active().turns[0].expanded);
    click(&mut rt, "theme");
    assert_eq!(rt.state.dark, Some(false));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.inspector.is_open());
    click(&mut rt, "diff-mode");
    assert_eq!(rt.state.diff.mode(), rune_kit::diff::DiffMode::Split);

    let (x, y) = rt.find("sidebar-grip").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_move(x + 40.0, y);
    rt.pointer_up(x + 40.0, y);
    advance(&mut rt, 0.5);
    assert_eq!(rt.state.sidebar.width(), 278.0);
    key(
        &mut rt,
        Key::Command('b'),
        Mods {
            cmd: true,
            shift: true,
            ..Mods::default()
        },
    );
    advance(&mut rt, 0.5);
    assert!(!rt.state.sidebar.is_open());
    click(&mut rt, "sidebar-toggle");
    advance(&mut rt, 0.5);
    assert!(rt.state.sidebar.is_open());
    assert_eq!(rt.state.sidebar.width(), 278.0);
    assert_eq!(rt.state.active().draft.text, draft);
    assert_eq!(rt.state.active().turns[0].response, response);
}

#[test]
fn compact_layout_keeps_the_composer_and_pane_controls_visible() {
    let mut rt = runtime((800.0, 620.0));
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    for key in [
        "draft-0",
        "send",
        "sidebar-toggle",
        "inspector-toggle",
        "close-inspector",
    ] {
        let e = element(&rt, key);
        assert!(
            e.rect[2] > 0.0 && e.rect[3] > 0.0,
            "{key} has a usable size"
        );
        for (actual, visible) in e.rect.into_iter().zip(e.visible) {
            assert!(
                (actual - visible).abs() < 1.0,
                "{key} is clipped: {:?} vs {:?}",
                e.rect,
                e.visible
            );
        }
    }
    click(&mut rt, "new-session");
    rt.paste("A new draft");
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.selected, 2);
    assert_eq!(
        rt.state.active().draft.text,
        "A new draft",
        "new sessions focus their composer"
    );
}

#[test]
fn scrolling_up_during_a_reply_preserves_the_reading_position() {
    let mut rt = runtime((1000.0, 700.0));
    click(&mut rt, "send");
    advance(&mut rt, 1.0);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
    let (x, y) = rt.find("transcript-0").unwrap();
    rt.wheel(x, y, 250.0);
    advance(&mut rt, 0.1);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(false));
    let reading = element(&rt, "turn-0").rect[1];
    advance(&mut rt, 6.0);
    assert_eq!(element(&rt, "turn-0").rect[1], reading);
    rt.wheel(x, y, -10000.0);
    advance(&mut rt, 0.5);
    assert_eq!(rt.scroll_at_bottom("transcript-0"), Some(true));
}

fn live_runtime() -> Runtime<Workspace> {
    live_runtime_size((1280.0, 1000.0))
}

#[test]
fn launcher_reuses_the_selected_project_draft_and_returns_keyboard_focus() {
    let mut rt = live_runtime();
    fill(&mut rt, "live-draft-0", "Keep my original draft");
    key(
        &mut rt,
        Key::Command('n'),
        Mods {
            cmd: true,
            shift: true,
            ..Default::default()
        },
    );
    advance(&mut rt, 0.3);
    click(&mut rt, "launch-provider-opencode");
    assert_eq!(
        rt.state.live.as_ref().unwrap().panels.launch_provider,
        caelum_core::application::Provider::OpenCode
    );
    fill(&mut rt, "launch-search", "project");
    click(&mut rt, &super::code_view::source_key("/project", "launch"));
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
    rt.paste("OpenCode draft");
    advance(&mut rt, 0.1);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "OpenCode draft"
    );
    key(
        &mut rt,
        Key::Command('n'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert_eq!(rt.state.live.as_ref().unwrap().app.sessions.len(), 2);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep my original draft"
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[1].text,
        "OpenCode draft"
    );
}

#[test]
fn review_refresh_and_tab_navigation_preserve_exact_patch_reading_position() {
    let mut state = super::evaluation::workspace();
    let l = state.live.as_mut().unwrap();
    l.evaluation = false;
    l.fixture = true;
    super::panels::open_review(
        &mut state,
        "/evaluation".into(),
        "https://github.com/example/evaluation/pull/1".into(),
    );
    let app = spec();
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    advance(&mut rt, 0.3);
    click(&mut rt, "review-tab-1");
    rt.update(|_, tx| tx.scroll_to_y("diff/scroll", 1000.0));
    advance(&mut rt, 0.3);
    let offset = |rt: &Runtime<Workspace>| {
        let elements = rt.elements();
        let view = elements
            .iter()
            .find(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
            .unwrap();
        let rows = elements
            .iter()
            .find(|e| e.path.ends_with("/diff/scroll/rows") && !e.exiting)
            .unwrap();
        rows.scrolled[1] - view.scrolled[1]
    };
    let before = offset(&rt);
    assert!(before > 900.0);
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let r = l.app.services.reviews.values_mut().next().unwrap();
        r.files[1].patch.as_mut().unwrap().push('\n');
        l.app.services.revision += 1;
    });
    advance(&mut rt, 0.3);
    assert!((offset(&rt) - before).abs() < 1.0);
    let files = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .services
        .reviews
        .values()
        .next()
        .unwrap()
        .files
        .clone();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.services.refresh_review(
            "/evaluation",
            "https://github.com/example/evaluation/pull/1",
        );
    });
    advance(&mut rt, 0.3);
    rt.update(|s, _| {
        let services = &mut s.live.as_mut().unwrap().app.services;
        let review = services.reviews.values_mut().next().unwrap();
        review.files = files.clone();
        review.files_loaded = true;
        services.revision += 1;
    });
    advance(&mut rt, 0.3);
    assert!(
        (offset(&rt) - before).abs() < 1.0,
        "explicit refresh restores the continuous patch position: {} → {}",
        before,
        offset(&rt)
    );
    rt.update(|s, _| s.live.as_mut().unwrap().panels.review_tab = 0);
    advance(&mut rt, 0.2);
    rt.update(|s, _| s.live.as_mut().unwrap().panels.review_tab = 1);
    advance(&mut rt, 0.3);
    assert!(
        (offset(&rt) - before).abs() < 1.0,
        "returning to files restores the continuous patch position"
    );
    fill(&mut rt, "review-file-search", "src/file-100.rs");
    click(&mut rt, "review-file-src/file-100.rs");
    advance(&mut rt, 0.3);
    let before = offset(&rt);
    assert!(before > 100_000.0, "later file offset: {before}");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.services.refresh_review(
            "/evaluation",
            "https://github.com/example/evaluation/pull/1",
        );
    });
    advance(&mut rt, 0.2);
    rt.update(|s, _| {
        let services = &mut s.live.as_mut().unwrap().app.services;
        let review = services.reviews.values_mut().next().unwrap();
        review.files = files[..25].to_vec();
        review.files_loaded = true;
        review.next_page = Some(2);
        services.revision += 1;
    });
    advance(&mut rt, 0.3);
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .panels
            .restoring_review()
            .is_some()
    );
    rt.update(|s, _| {
        let services = &mut s.live.as_mut().unwrap().app.services;
        let review = services.reviews.values_mut().next().unwrap();
        review.files = files.clone();
        review.next_page = None;
        services.revision += 1;
    });
    advance(&mut rt, 0.3);
    assert!(
        (offset(&rt) - before).abs() < 1.0,
        "paginated refresh: {before} → {}",
        offset(&rt)
    );
}

#[test]
fn desktop_shutdown_saves_the_latest_editor_and_releases_the_workspace_lock() {
    use caelum_core::application::{Provider, Scope, Store, Workspace as Application};
    use std::{
        fs,
        sync::{Arc, mpsc},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let directory = std::env::temp_dir().join(format!(
        "caelum-desktop-shutdown-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = Store(directory.join("workspace.json"));
    let mut app = Application::default();
    app.new_session(Scope {
        project: "/project".into(),
        provider: Provider::Codex,
    });
    let mut saved = app.snapshot();
    saved.selected = None; // Restore offline drafts without contacting a provider.
    saved.sessions[0].draft = "saved draft".into();
    store.save(&saved).unwrap();
    let (send, receive) = mpsc::channel();
    let mut live = super::live::Live::new(None);
    live.start(
        store.clone(),
        Arc::new(move || {
            let _ = send.send(());
        }),
    );
    assert!(store.lock().is_err());
    live.poll(&mut rune::Tx::default());
    while live.drafts.is_empty() {
        receive
            .recv_timeout(Duration::from_secs(5))
            .expect("workspace load wakes the UI");
        live.poll(&mut rune::Tx::default());
    }
    assert_eq!(live.drafts[0].text, "saved draft");
    live.drafts[0].text = "older in-flight save".into();
    live.poll(&mut rune::Tx::default());
    live.drafts[0].text = "latest edit before quit".into();
    drop(live);
    assert_eq!(
        store.load().unwrap().unwrap().sessions[0].draft,
        "latest edit before quit"
    );
    assert!(store.lock().is_ok());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn project_links_wait_for_restore_and_keep_drafts_when_the_app_is_running() {
    use caelum_core::application::{
        Command, Event, Provider, Reply, Scope, Workspace as Application,
    };
    let mut original = Application::default();
    original.projects.push(caelum_protocol::ProjectInfo {
        name: "Original".into(),
        path: "/original".into(),
    });
    original.new_session(Scope {
        project: "/original".into(),
        provider: Provider::Codex,
    });
    original.sessions[0].draft = "restored draft".into();
    let mut state = Workspace::live(None);
    state
        .live
        .as_mut()
        .unwrap()
        .app
        .enable_store("/unused-link-test/workspace.json".into());
    let url = caelum_core::desktop::project_url(std::path::Path::new("/next café")).unwrap();
    super::lifecycle::open_url(&mut state, &url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    let commands = live.app.commands();
    assert_eq!(commands.len(), 1);
    assert!(matches!(commands[0].1, Command::Load(_)));
    live.app.apply(Event::Completed(
        commands[0].0,
        Ok(Reply::Snapshot(Some(Box::new(original.snapshot())))),
    ));
    live.poll(&mut rune::Tx::default());
    let mut commands = live.app.commands();
    if let Some((save, _)) = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
    {
        live.app.apply(Event::Completed(*save, Ok(Reply::Done)));
        commands = live.app.commands();
    }
    let open = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Open(path) if path == "/next café"))
        .unwrap()
        .0;
    live.app.apply(Event::Completed(
        open,
        Ok(Reply::Project(caelum_protocol::ProjectInfo {
            name: "Next".into(),
            path: "/next café".into(),
        })),
    ));
    live.poll(&mut rune::Tx::default());
    assert_eq!(live.drafts[0].text, "restored draft");
    assert_eq!(live.app.active_project.as_deref(), Some("/next café"));
    live.drafts[1].text = "draft in the running app".into();
    let original_url =
        caelum_core::desktop::project_url(std::path::Path::new("/original")).unwrap();
    super::lifecycle::open_url(&mut state, &original_url, &mut rune::Tx::default());
    let live = state.live.as_mut().unwrap();
    live.poll(&mut rune::Tx::default());
    assert_eq!(live.drafts[1].text, "draft in the running app");
    super::lifecycle::open_url(
        &mut state,
        "caelum://open?path=relative",
        &mut rune::Tx::default(),
    );
    assert!(state.live.as_ref().unwrap().app.error.is_some());
    // This reducer-only instance must not try to write the sentinel path on drop.
    let live = state.live.as_mut().unwrap();
    live.app = Application::default();
    live.drafts.clear();
}

#[test]
fn project_url_wakes_an_idle_app_to_process_the_open_request() {
    use caelum_core::application::Store;
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    let directory = std::env::temp_dir().join(format!(
        "caelum-project-url-wake-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let wake_count = Arc::new(AtomicUsize::new(0));
    let mut state = Workspace::live(None);
    state
        .live
        .as_mut()
        .unwrap()
        .start(Store(directory.join("workspace.json")), {
            let wake_count = Arc::clone(&wake_count);
            Arc::new(move || {
                wake_count.fetch_add(1, Ordering::SeqCst);
            })
        });
    let before = wake_count.load(Ordering::SeqCst);
    let url = caelum_core::desktop::project_url(std::path::Path::new("/next-project")).unwrap();

    super::lifecycle::open_url(&mut state, &url, &mut rune::Tx::default());

    assert!(wake_count.load(Ordering::SeqCst) > before);
    drop(state);
    fs::remove_dir_all(directory).unwrap();
}

fn live_runtime_size(size: (f32, f32)) -> Runtime<Workspace> {
    use caelum_core::application::{Provider, Scope};
    let mut state = Workspace::live(None);
    let live = state.live.as_mut().unwrap();
    live.app.projects.push(caelum_protocol::ProjectInfo {
        name: "Project".into(),
        path: "/project".into(),
    });
    for provider in Provider::ALL {
        live.app.new_session(Scope {
            project: "/project".into(),
            provider,
        });
    }
    live.app.select(0);
    live.poll(&mut rune::Tx::default());
    let app = spec();
    let mut rt = Runtime::new(state, app.view, size, Env::new(true, 1.0));
    rt.poll = app.poll;
    rt.on_key = app.on_key;
    rt.set_font_families(app.families.0, app.families.1);
    advance(&mut rt, 0.5);
    rt
}

fn close_session(rt: &mut Runtime<Workspace>, index: usize) {
    let [x, y, width, height] = element(rt, &format!("live-session-{index}")).rect;
    // The hover close control is the trailing 20-point slot of this row.
    let (x, y) = (x + width - 18.0, y + height / 2.0);
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    advance(rt, 0.3);
}

#[test]
fn closing_local_sessions_and_undo_keep_the_workspace_editable() {
    for dialog in [false, true] {
        let mut rt = live_runtime();
        click(&mut rt, "live-draft-0");
        rt.paste("Keep the archived draft café ☁️");
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste("Independent background draft");
        click(&mut rt, "live-session-0");
        if dialog {
            click(&mut rt, "session-actions-0");
            click(&mut rt, "archive-session");
            advance(&mut rt, 0.4);
        } else {
            close_session(&mut rt, 0);
        }
        let live = rt.state.live.as_ref().unwrap();
        assert!(live.app.sessions[0].archived);
        assert_eq!(live.app.selected, Some(2));
        assert!(live.panels.overlay.is_none());
        element(&rt, "live-draft-2");
        rt.paste("Replacement draft");
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[2].text,
            "Replacement draft"
        );
        close_session(&mut rt, 1);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(2));
        click(&mut rt, "undo-archive");
        let live = rt.state.live.as_ref().unwrap();
        assert_eq!(live.app.selected, Some(2));
        assert!(!live.app.sessions[1].archived);
        assert_eq!(live.drafts[1].text, "Independent background draft");
        assert_eq!(live.drafts[0].text, "Keep the archived draft café ☁️");
        assert_eq!(live.drafts[2].text, "Replacement draft");
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste(" after undo");
        rt.frame(1.0 / 60.0);
        assert!(
            rt.state.live.as_ref().unwrap().drafts[1]
                .text
                .ends_with(" after undo")
        );

        click(&mut rt, "toggle-archived");
        advance(&mut rt, 0.4);
        click(&mut rt, "live-session-0");
        click(&mut rt, "session-actions-0");
        click(&mut rt, "archive-session");
        let live = rt.state.live.as_ref().unwrap();
        assert!(!live.panels.archived);
        assert!(!live.app.sessions[0].archived);
        assert_eq!(live.app.selected, Some(0));
        assert_eq!(live.drafts[0].text, "Keep the archived draft café ☁️");
    }
}

#[test]
fn closing_provider_sessions_handles_completion_failure_and_selection_changes() {
    use caelum_core::application::{Command, Event, Provider, Reply};
    for provider in Provider::ALL {
        let mut rt = live_runtime();
        rt.update(|s, _| {
            let session = &mut s.live.as_mut().unwrap().app.sessions[0];
            session.scope.provider = provider;
            session.remote_id = Some("remote-session".into());
            session.loaded = true;
        });
        click(&mut rt, "live-draft-0");
        rt.paste("Provider draft");
        close_session(&mut rt, 0);
        let live = rt.state.live.as_mut().unwrap();
        assert!(live.app.sessions[0].archiving);
        assert_eq!(live.app.selected, Some(0));
        let (request, command) = live.app.commands().pop().unwrap();
        assert!(matches!(command, Command::Archive(key, true)
            if key.scope.provider == provider && key.id == "remote-session"));
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Err("Offline".into())));
        });
        let session = &rt.state.live.as_ref().unwrap().app.sessions[0];
        assert!(!session.archived && !session.archiving);
        assert_eq!(session.error.as_deref(), Some("Offline"));
        element(&rt, "live-draft-0");

        // Completing a background archive must not replace the user's new selection.
        close_session(&mut rt, 0);
        let request = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap()
            .0;
        click(&mut rt, "live-session-1");
        click(&mut rt, "live-draft-1");
        rt.paste("Still writing here");
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
        assert_eq!(rt.state.live.as_ref().unwrap().app.sessions.len(), 2);
        assert_eq!(rt.state.live.as_ref().unwrap().app.last_archived, Some(0));
        advance(&mut rt, 0.4);
        click(&mut rt, "undo-archive");
        let (request, command) = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap();
        assert!(matches!(command, Command::Archive(_, false)));
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        advance(&mut rt, 0.4);
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(1));
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[1].text,
            "Still writing here"
        );
        click(&mut rt, "live-session-0");
        click(&mut rt, "live-draft-0");
        rt.paste(" after restore");
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Provider draft after restore"
        );

        // The selected provider session also creates a ready-to-type replacement.
        click(&mut rt, "session-actions-0");
        click(&mut rt, "archive-session");
        let request = rt
            .state
            .live
            .as_mut()
            .unwrap()
            .app
            .commands()
            .pop()
            .unwrap()
            .0;
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Ok(Reply::Done)));
        });
        advance(&mut rt, 0.4);
        element(&rt, "live-draft-2");
        rt.paste("New provider draft");
        rt.frame(1.0 / 60.0);
        let live = rt.state.live.as_ref().unwrap();
        assert_eq!(live.app.selected, Some(2));
        assert_eq!(live.drafts[2].text, "New provider draft");
        assert_eq!(live.drafts[1].text, "Still writing here");
    }
}

#[test]
fn live_creation_streaming_and_failed_submissions_preserve_independent_editors() {
    use caelum_core::application::{Command, Event, Options, Reply, SessionKey, Summary};
    let mut rt = live_runtime();
    click(&mut rt, "live-draft-0");
    rt.paste("First real prompt");
    key(&mut rt, Key::Enter, Mods::default());
    let (request, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    let Command::Start(scope, _) = command else {
        panic!("expected creation")
    };
    rt.paste("Next draft");
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-draft-1");
    rt.paste("Other provider draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Ok(Reply::Session(Summary {
                key: SessionKey {
                    scope,
                    id: "thread-1".into(),
                },
                title: "Created".into(),
                working: false,
                options: Options::default(),
            })),
        ))
    });
    let (send, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(matches!(command, Command::Send(_, ref prompt, _) if prompt == "First real prompt"));
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.apply(Event::Completed(send, Ok(Reply::Done)));
        app.apply(Event::Codex(caelum_protocol::CodexEvent {
            project_path: "/project".into(), method: "item/agentMessage/delta".into(), request_id: None,
            params: serde_json::json!({ "threadId": "thread-1", "itemId": "answer", "delta": "Background response" }),
        }));
    });
    advance(&mut rt, 0.5);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.selected, Some(1));
    assert_eq!(live.drafts[0].text, "Next draft");
    assert_eq!(live.drafts[1].text, "Other provider draft");
    assert_eq!(
        live.app.sessions[0].conversation.entries[1].text,
        "Background response"
    );
    key(&mut rt, Key::Enter, Mods::default());
    let (request, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    rt.paste("Newer unsent draft");
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            request,
            Err("Provider unavailable".into()),
        ))
    });
    advance(&mut rt, 0.2);
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.drafts[1].text, "Newer unsent draft");
    assert_eq!(
        live.app.sessions[1].failed_prompt.as_deref(),
        Some("Other provider draft")
    );
    click(&mut rt, "live-session-0");
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, "Next draft");
}

#[test]
fn live_navigation_restores_reading_position_and_approval_decisions_keep_their_owner() {
    use caelum_core::application::{Command, Event, Provider};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.sessions[0].remote_id = Some("thread-1".into());
        app.sessions[0].conversation.history(Provider::Codex, &[serde_json::json!({
            "status":"completed", "items":[{"id":"long","type":"agentMessage","text":"A paragraph worth reading.\n\n".repeat(90)}]
        })], false);
    });
    advance(&mut rt, 0.5);
    let (x, y) = rt.find("live-transcript-0").unwrap();
    rt.wheel(x, y, 300.0);
    advance(&mut rt, 0.2);
    let position = element(&rt, "live-entry-0-codex-long").rect[1];
    click(&mut rt, "live-session-1");
    click(&mut rt, "live-session-0");
    advance(&mut rt, 0.5);
    assert!((element(&rt, "live-entry-0-codex-long").rect[1] - position).abs() < 1.0);
    assert_eq!(rt.scroll_at_bottom("live-transcript-0"), Some(false));
    rt.update(|s, tx| {
        s.live.as_mut().unwrap().app.apply(Event::Approvals(vec![
            caelum_protocol::ApprovalPrompt {
                provider: "codex".into(),
                project_path: "/project".into(),
                session_id: Some("thread-1".into()),
                request_id: "one".into(),
                summary: "Run command?".into(),
                description: "cargo test".into(),
            },
        ]));
        tx.scroll_to_bottom_instant("live-transcript-0");
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "allow-once-one");
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Approve(prompt, caelum_protocol::ApprovalDecision::AllowOnce)
        if prompt.project_path == "/project" && prompt.session_id.as_deref() == Some("thread-1") && prompt.request_id == "one")
    );
}

#[test]
#[ignore = "requires a locally installed/authenticated provider and CAELUM_RUNE_SMOKE_PROJECT"]
fn real_provider_discovery_and_history_smoke() {
    use caelum_core::application::Provider;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let path = std::env::var("CAELUM_RUNE_SMOKE_PROJECT")
        .expect("set a project containing provider history");
    std::env::var_os("CAELUM_STATE")
        .or_else(|| std::env::var_os("CAELUM_RUNE_STATE"))
        .expect("set an isolated CAELUM_STATE path for smoke checks");
    let provider = if std::env::var("CAELUM_RUNE_SMOKE_PROVIDER").as_deref() == Ok("codex") {
        Provider::Codex
    } else {
        Provider::OpenCode
    };
    let app = spec();
    let mut state = Workspace::live(Some(path));
    app.setup.unwrap()(&mut state, Arc::new(|| {}));
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut selected = None;
    loop {
        rt.frame(1.0 / 60.0);
        let live = rt.state.live.as_ref().unwrap();
        assert!(
            live.app.error.is_none(),
            "project failed: {:?}",
            live.app.error
        );
        if let Some(error) = live
            .app
            .notices
            .iter()
            .find(|(scope, _)| scope.provider == provider)
            .map(|(_, error)| error)
        {
            panic!("provider failed: {error}");
        }
        if let Some(index) = selected {
            let session: &caelum_core::application::Session = &live.app.sessions[index];
            assert!(
                session.error.is_none(),
                "history failed: {:?}",
                session.error
            );
            if session.loaded && !session.loading {
                assert!(
                    !session.conversation.entries.is_empty(),
                    "choose a project with nonempty history"
                );
                println!(
                    "{}: {} sessions discovered; {} history entries rendered",
                    provider.as_str(),
                    live.app.sessions.len(),
                    session.conversation.entries.len()
                );
                break;
            }
        } else if let Some(index) = live
            .app
            .sessions
            .iter()
            .position(|s| s.scope.provider == provider && s.remote_id.is_some())
        {
            selected = Some(index);
            rt.update(|s, _| s.live.as_mut().unwrap().app.select(index));
        }
        assert!(
            Instant::now() < deadline,
            "provider discovery/history timed out"
        );
        std::thread::sleep(Duration::from_millis(16));
    }
}

#[test]
#[ignore = "requires a real Git checkout, authenticated gh and CAELUM_RUNE_SMOKE_PR"]
fn real_workspace_services_smoke() {
    use caelum_core::application::{
        Command, Event, Reply, Runtime as WorkerRuntime, Task, TaskReply,
    };
    use caelum_protocol::{GitHubFeedbackKind, ProjectSearchMode};
    use std::time::{Duration, Instant};
    let project = std::env::var("CAELUM_RUNE_SMOKE_PROJECT").expect("set a real checkout");
    let url = std::env::var("CAELUM_RUNE_SMOKE_PR").expect("set a pull request URL");
    let mut runtime = WorkerRuntime::new(|| {});
    let mut id = 0;
    let mut run = |task: Task| {
        id += 1;
        runtime.submit(id, Command::Task(task)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            for event in runtime.drain() {
                if let Event::Completed(current, result) = event
                    && current == id
                {
                    let Reply::Task(reply) = result.expect("workspace service failed") else {
                        panic!("wrong reply")
                    };
                    return reply;
                }
            }
            assert!(Instant::now() < deadline, "workspace service timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    assert!(
        matches!(run(Task::Directory { project: project.clone(), path: String::new(), offset: 0 }), TaskReply::Directory(page) if !page.entries.is_empty())
    );
    assert!(
        matches!(run(Task::File { project: project.clone(), path: "Cargo.toml".into() }), TaskReply::File(file) if !file.content.is_empty())
    );
    assert!(
        matches!(run(Task::Search { project: project.clone(), query: "Cargo.toml".into(), mode: ProjectSearchMode::Files }), TaskReply::Search(page) if !page.matches.is_empty())
    );
    let TaskReply::Git(status, _) = run(Task::Git {
        project: project.clone(),
    }) else {
        panic!("Git status")
    };
    assert!(
        matches!(run(Task::Worktrees { project: project.clone(), directory: String::new() }), TaskReply::Worktrees(Some(list)) if !list.worktrees.is_empty())
    );
    if let Some(change) = status.changes.first() {
        assert!(matches!(
            run(Task::Diff {
                project: project.clone(),
                path: change.path.clone()
            }),
            TaskReply::Diff(_)
        ));
    }
    let TaskReply::Details(details) = run(Task::Details {
        project: project.clone(),
        url: url.clone(),
    }) else {
        panic!("PR overview")
    };
    assert!(matches!(
        run(Task::Discover {
            project: project.clone(),
            branch: details.head_ref_name.clone()
        }),
        TaskReply::Discovered(Some(_))
    ));
    let head = details.head_ref_oid;
    let TaskReply::ReviewFiles(files) = run(Task::ReviewFiles {
        project: project.clone(),
        url: url.clone(),
        head: head.clone(),
        page: 1,
    }) else {
        panic!("PR files")
    };
    assert!(!files.files.is_empty());
    for kind in [
        GitHubFeedbackKind::Discussion,
        GitHubFeedbackKind::Reviews,
        GitHubFeedbackKind::Threads,
    ] {
        assert!(matches!(
            run(Task::Feedback {
                project: project.clone(),
                url: url.clone(),
                head: head.clone(),
                kind,
                cursor: None,
                thread: None
            }),
            TaskReply::Feedback(_)
        ));
    }
    println!(
        "Real workspace: directory, file, search, Git/diff, worktrees, PR discovery/overview, {} changed files and all feedback categories loaded",
        files.files.len()
    );
}

#[test]
fn settings_provider_and_composer_choices_apply_to_first_send() {
    use caelum_core::application::{Command, Provider, Scope};
    use caelum_protocol::{ProviderChoice, ProviderChoices, ProviderModelChoice};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let app = &mut s.live.as_mut().unwrap().app;
        app.choices.insert(
            Scope {
                provider: Provider::OpenCode,
                project: "/project".into(),
            },
            ProviderChoices {
                models: ["first", "second"]
                    .into_iter()
                    .map(|id| ProviderModelChoice {
                        id: id.into(),
                        name: id.into(),
                        provider: None,
                        context_window: None,
                        efforts: vec![ProviderChoice {
                            id: "low".into(),
                            name: "Low".into(),
                        }],
                        default_effort: Some("low".into()),
                    })
                    .collect(),
                agents: vec![ProviderChoice {
                    id: "build".into(),
                    name: "Build".into(),
                }],
                default_model: Some("first".into()),
            },
        );
    });
    fill(&mut rt, "live-draft-0", "Keep this Codex draft");
    click(&mut rt, "nav-settings");
    click(&mut rt, "settings-providers");
    click(&mut rt, "default-provider-opencode");
    key(&mut rt, Key::Escape, Mods::default());
    click(&mut rt, "new-session");
    let index = rt.state.live.as_ref().unwrap().app.selected.unwrap();
    let draft = format!("live-draft-{index}");
    assert_ne!(index, 0);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep this Codex draft"
    );
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[0]
            .scope
            .provider,
        Provider::Codex
    );
    assert!(rt.find("provider-trigger").is_none());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[index]
            .scope
            .provider,
        Provider::OpenCode
    );
    click(&mut rt, "model-trigger");
    fill(&mut rt, "model-search", "second");
    click(&mut rt, "model-favorite-0");
    assert!(element(&rt, "model-search").focused);
    assert_eq!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .preferences
            .favorite_models["opencode"],
        vec!["second"]
    );
    key(&mut rt, Key::Enter, Mods::default());
    assert!(element(&rt, &draft).focused);
    click(&mut rt, "model-trigger");
    fill(&mut rt, "model-search", "first");
    key(&mut rt, Key::Escape, Mods::default());
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.sessions[index]
            .options
            .model
            .as_deref(),
        Some("second")
    );
    click(&mut rt, "model-trigger");
    fill(&mut rt, "model-search", "second");
    click(&mut rt, "option0");
    click(&mut rt, "agent-trigger");
    key(&mut rt, Key::End, Mods::default());
    key(&mut rt, Key::Enter, Mods::default());
    click(&mut rt, &draft);
    rt.paste("Use my choices");
    key(&mut rt, Key::Enter, Mods::default());
    let (_, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(&command, Command::Start(scope, options) if scope.provider == Provider::OpenCode
        && options.model.as_deref() == Some("second") && options.effort.as_deref() == Some("low") && options.agent.as_deref() == Some("build")),
        "{command:?}"
    );
    click(&mut rt, "model-trigger");
    fill(&mut rt, "model-search", "first");
    click(&mut rt, "option0");
    click(&mut rt, &draft);
    rt.paste("Queue with a different model");
    key(&mut rt, Key::Enter, Mods::default());
    let queued = &rt.state.live.as_ref().unwrap().app.sessions[index].queue;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].options.model.as_deref(), Some("first"));
}

#[test]
fn compact_live_inspector_and_modals_preserve_the_composer_and_focus() {
    let mut rt = live_runtime_size((800.0, 620.0));
    click(&mut rt, "live-draft-0");
    rt.paste("Keep this compact draft");
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    for screen in ["nav-files", "nav-changes"] {
        click(&mut rt, screen);
        for key in ["live-draft-0", "send", "close-inspector", "model-trigger"] {
            let e = element(&rt, key);
            assert!(e.rect[2] > 0.0 && e.rect[3] > 0.0);
            assert!(
                e.rect
                    .into_iter()
                    .zip(e.visible)
                    .all(|(a, b)| (a - b).abs() < 1.0),
                "{screen}: {key} is clipped: {:?} vs {:?}",
                e.rect,
                e.visible
            );
        }
    }
    click(&mut rt, "close-inspector");
    advance(&mut rt, 0.5);
    for screen in ["nav-worktrees", "nav-settings", "session-actions-0"] {
        click(&mut rt, screen);
        let dialog = element(
            &rt,
            if screen == "session-actions-0" {
                "session-menu"
            } else {
                "workspace-dialog"
            },
        );
        assert!(
            dialog
                .rect
                .into_iter()
                .zip(dialog.visible)
                .all(|(a, b)| (a - b).abs() < 1.0)
        );
        click(&mut rt, "live-draft-0");
        rt.paste("must not enter the background composer");
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Keep this compact draft"
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.4);
        assert!(rt.state.live.as_ref().unwrap().panels.overlay.is_none());
    }
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Keep this compact draft"
    );
}

#[test]
fn two_thousand_entries_and_six_streams_keep_the_native_tree_bounded() {
    let app = spec();
    let mut rt = Runtime::new(
        crate::evaluation::workspace(),
        app.view,
        (1280.0, 1000.0),
        Env::new(true, 1.0),
    );
    rt.poll = app.poll;
    advance(&mut rt, 0.6);
    let mut times = Vec::new();
    for _ in 0..120 {
        let start = std::time::Instant::now();
        rt.frame(1.0 / 60.0);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    let mounted = rt
        .elements()
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|key| key.starts_with("live-entry-"))
        })
        .count();
    assert!(
        mounted < 120,
        "history must be virtualized, mounted {mounted}"
    );
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.sessions[0].conversation.entries.len(), 2001);
    assert!(live.app.sessions.iter().all(|s| s.working()));
    println!(
        "2,000 entries × 6 sessions: {mounted} mounted entries; CPU frame p50 {:.2} ms, p95 {:.2} ms (headless, not GPU presentation)",
        times[60], times[114]
    );
    click(&mut rt, "live-draft-0");
    rt.paste("Draft remains editable while six sessions stream");
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.live.as_ref().unwrap().drafts[0].text,
        "Draft remains editable while six sessions stream"
    );
}

#[test]
fn native_question_forms_send_scoped_values_and_keep_invalid_answers_editable() {
    use caelum_core::application::{Command, Event};
    let mut rt = live_runtime();
    rt.update(|s, _| {
        let live = s.live.as_mut().unwrap(); live.app.sessions[0].remote_id = Some("thread".into());
        live.app.apply(Event::Questions(vec![caelum_protocol::QuestionPrompt { provider: "codex".into(), project_path: "/project".into(), session_id: "thread".into(), request_id: "question".into(), title: "Choose a branch".into(), fields: vec![serde_json::json!({"key":"branch","type":"string","required":true,"options":[{"label":"Main","value":"main"},{"label":"Feature","value":"feature"}]})] }]));
    });
    advance(&mut rt, 0.3);
    click(&mut rt, "question-submit-question");
    assert!(rt.state.live.as_mut().unwrap().app.commands().is_empty());
    click(&mut rt, "question-question-branch-1");
    click(&mut rt, "question-submit-question");
    assert!(
        matches!(rt.state.live.as_mut().unwrap().app.commands().as_slice(), [(_, Command::Answer(prompt, Some(answer)))] if prompt.session_id == "thread" && answer["branch"] == "feature")
    );
}

#[test]
fn native_file_preview_attaches_an_immutable_snapshot_to_the_originating_draft() {
    use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
    let mut rt = live_runtime();
    click(&mut rt, "inspector-toggle");
    advance(&mut rt, 0.5);
    let (directory, command) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    assert!(
        matches!(command, Command::Task(Task::Directory { ref project, .. }) if project == "/project")
    );
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            directory,
            Ok(Reply::Task(TaskReply::Directory(
                caelum_protocol::ProjectDirectoryListing {
                    path: "".into(),
                    entries: vec![caelum_protocol::ProjectFileEntry {
                        name: "main.rs".into(),
                        path: "main.rs".into(),
                        kind: caelum_protocol::ProjectEntryKind::File,
                        size_bytes: Some(12),
                    }],
                    total: 1,
                    next_offset: None,
                },
            ))),
        ))
    });
    advance(&mut rt, 0.3);
    let file_row = element(&rt, "file-tree-main.rs");
    assert!(
        file_row.visible[2] > 100.0 && file_row.visible[3] >= 28.0,
        "file row: {file_row:?}"
    );
    click(&mut rt, "file-tree-main.rs");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.services.checkouts["/project"]
            .selected_file
            .as_deref(),
        Some("main.rs")
    );
    let (file, _) = rt
        .state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .pop()
        .unwrap();
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.apply(Event::Completed(
            file,
            Ok(Reply::Task(TaskReply::File(
                caelum_protocol::ProjectFileContent {
                    path: "main.rs".into(),
                    content: "fn main() {}".into(),
                    size_bytes: 12,
                    truncated: false,
                },
            ))),
        ))
    });
    advance(&mut rt, 0.3);
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let gutter = rt
        .elements()
        .into_iter()
        .find(|e| e.path.contains("source-") && e.path.ends_with("/df0.c1/gutter/nn"))
        .unwrap();
    let [x, y, w, h] = gutter.rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    advance(&mut rt, 0.2);
    key(
        &mut rt,
        Key::Command('c'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert!(clipboard.text().unwrap().contains("fn main() {}"));
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .working = true
    });
    key(&mut rt, Key::Escape, Mods::default());
    assert!(
        !rt.state.live.as_ref().unwrap().app.sessions[0].stopping,
        "Escape clears the file selection before cancelling work"
    );
    assert_eq!(
        element(&rt, "attach-preview").label.as_deref(),
        Some("Add to prompt")
    );
    click(&mut rt, "attach-preview");
    click(&mut rt, "close-inspector");
    click(&mut rt, "live-session-1");
    let live = rt.state.live.as_ref().unwrap();
    assert_eq!(live.app.sessions[0].attachments[0].content, "fn main() {}");
    assert!(live.app.sessions[1].attachments.is_empty());
}

#[test]
fn large_review_files_are_windowed_while_streams_continue() {
    let app = spec();
    let mut state = crate::evaluation::workspace();
    let services = &mut state.live.as_mut().unwrap().app.services;
    services.reviews.values_mut().next().unwrap().threads = (0..12).map(|i| {
        serde_json::from_value(serde_json::json!({
            "id":format!("outdated-{i}"), "path":"src/file-0.rs", "line":null,
            "startLine":null, "originalLine":2, "diffSide":"RIGHT",
            "isResolved":false, "isOutdated":true, "nextCursor":null,
            "comments":[{"id":format!("comment-{i}"), "body":"Keep this earlier review feedback accessible.",
                "url":"https://github.com/example/evaluation/pull/1", "author":null,
                "createdAt":"2026-11-07T10:00:00Z", "state":null, "diffHunk":null}]
        })).unwrap()
    }).collect();
    services.revision += 1;
    crate::panels::open_review(
        &mut state,
        "/evaluation".into(),
        "https://github.com/example/evaluation/pull/1".into(),
    );
    let mut rt = Runtime::new(state, app.view, (1280.0, 1000.0), Env::new(true, 1.0));
    rt.poll = app.poll;
    advance(&mut rt, 0.5);
    click(&mut rt, "review-tab-1");
    let count = rt
        .elements()
        .iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("review-file-src/"))
        })
        .count();
    assert!(
        count > 0
            && count <= 3 * (element(&rt, "review-file-list").rect[3] / 36.0).ceil() as usize + 6,
        "only nearby file rows should be mounted: {count}"
    );
    click(&mut rt, "review-file-src/file-0.rs");
    advance(&mut rt, 0.3);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.path.ends_with("/diff/scroll") && e.rect[3] > 200.0),
        "the selected patch must have a usable viewport"
    );
    let notes = element(&rt, "review-file-notes");
    assert!(
        notes.visible[3] > 0.0 && notes.rect[1] + notes.rect[3] <= 1000.0,
        "outdated threads remain accessible alongside the patch"
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref().is_some_and(|t| t.contains("let value_0"))),
        "review must render code, not just the file list"
    );
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .app
            .sessions
            .iter()
            .all(|s| s.working())
    );
    click(&mut rt, "back-to-session");
    assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
}

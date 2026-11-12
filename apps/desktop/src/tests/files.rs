use super::*;

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
        crate::panels::show(s, crate::panels::Screen::Files);
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

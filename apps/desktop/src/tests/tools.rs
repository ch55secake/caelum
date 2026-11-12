use super::*;

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
fn tool_details_are_output_first_and_keep_failures_visible() {
    let _fonts = rune::text::with_families("Inter", "Inter");
    let mut rt = design_runtime("tools-collapsed", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "tool-0-opencode-tool-run");
    assert!(element(&rt, "tool-0-opencode-tool-run").expanded.unwrap());
    assert!(!rt.elements().iter().any(|e| {
        !e.exiting
            && e.text
                .as_deref()
                .is_some_and(|t| t == "Input" || t.contains("timeout") || t.contains("Exit 0"))
    }));
    let copy = rt
        .elements()
        .into_iter()
        .find(|e| e.label.as_deref() == Some("Copy output") && !e.exiting)
        .unwrap();
    click(&mut rt, &copy.path);
    assert_eq!(
        clipboard.text().unwrap().as_str(),
        "test result: ok. 190 passed; 0 failed"
    );
    click(&mut rt, "tool-0-opencode-tool-read");
    click(&mut rt, "open-tool-file-0-opencode-tool-read");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.services.checkouts["/projects/caelum"]
            .selected_file
            .as_deref(),
        Some("src/workspace.rs")
    );
    assert!(rt.state.inspector.is_open());
    let mut failed = design_runtime("tool-failure", (800.0, 620.0));
    for label in [
        "Exit 101",
        "A workspace test failed. Open the output for details.",
    ] {
        assert!(
            failed
                .elements()
                .iter()
                .any(|e| !e.exiting && e.text.as_deref() == Some(label) && e.visible[3] > 0.0)
        );
    }
    failed.use_clipboard(&clipboard);
    click(&mut failed, "tool-0-opencode-tool-run");
    click(&mut failed, "tool-output-0-opencode-tool-run-Output-copy");
    assert!(clipboard.text().unwrap().contains("319 passed; 1 failed"));
    click(&mut failed, "tool-output-0-opencode-tool-run-Error-copy");
    assert_eq!(
        clipboard.text().unwrap().as_str(),
        "A workspace test failed. Open the output for details."
    );
    failed.update(|s, _| {
        let entry = s.live.as_mut().unwrap().app.sessions[0]
            .conversation
            .entries
            .iter_mut()
            .find(|e| e.id == "opencode-tool-run")
            .unwrap();
        entry.output.clone_from(&entry.error);
    });
    advance(&mut failed, 0.3);
    assert!(
        failed
            .find("tool-output-0-opencode-tool-run-Output-copy")
            .is_none()
    );
    assert!(
        failed
            .find("tool-output-0-opencode-tool-run-Error-copy")
            .is_some()
    );
}

#[test]
fn long_tool_output_preserves_copy_selection_scrolling_and_expansion_anchor() {
    let _fonts = rune::text::with_families("Inter", "Inter");
    let mut rt = design_runtime("tool-output-long", (800.0, 620.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let raw = rt.state.live.as_ref().unwrap().app.sessions[0]
        .conversation
        .entries
        .iter()
        .find(|e| e.id == "opencode-tool-run")
        .unwrap()
        .output
        .clone();
    let text = "tool-output-0-opencode-tool-run-Output-text";
    let copy = "tool-output-0-opencode-tool-run-Output-copy";
    let toggle = "tool-output-0-opencode-tool-run-Output-expand";
    let preview = element(&rt, text).text.unwrap();
    assert!(preview.chars().count() <= 12_000);
    assert!(raw.starts_with(&preview) && preview.len() < raw.len());
    assert!(preview.contains("café"));
    assert!(raw.lines().any(|line| Some(line) == preview.lines().last()));
    click(&mut rt, copy);
    assert_eq!(clipboard.text().unwrap().as_str(), raw);

    let [x, y, _, _] = element(&rt, text).visible;
    rt.pointer_down(x + 2.0, y + 10.0);
    rt.pointer_move(x + 110.0, y + 10.0);
    rt.pointer_up(x + 110.0, y + 10.0);
    let selected = rt.selected_text().expect("tool output remains selectable");
    assert!(!selected.is_empty() && raw.contains(&selected));
    key(
        &mut rt,
        Key::Command('c'),
        Mods {
            cmd: true,
            ..Default::default()
        },
    );
    assert_eq!(clipboard.text().unwrap().as_str(), selected);

    let header_y = element(&rt, "tool-0-opencode-tool-run").rect[1];
    let text_y = element(&rt, text).rect[1];
    rt.wheel(x + 30.0, y + 30.0, -120.0);
    advance(&mut rt, 0.3);
    assert!(element(&rt, text).rect[1] < text_y - 50.0);
    assert!((element(&rt, "tool-0-opencode-tool-run").rect[1] - header_y).abs() < 1.0);

    let toggle_y = element(&rt, toggle).rect[1];
    click(&mut rt, toggle);
    advance(&mut rt, 0.5);
    assert_eq!(element(&rt, text).text.as_deref(), Some(raw.as_str()));
    assert!((element(&rt, toggle).rect[1] - toggle_y).abs() < 1.0);
    click(&mut rt, toggle);
    assert_eq!(element(&rt, text).text.as_deref(), Some(preview.as_str()));
    click(&mut rt, copy);
    assert_eq!(clipboard.text().unwrap().as_str(), raw);
}

#[test]
fn compaction_milestones_stream_expand_copy_and_survive_history_refresh() {
    use caelum_core::application::Provider;
    // Parallel runtimes can switch the process-wide text measurer's families.
    let _fonts = rune::text::with_families("Inter", "Inter");
    let mut rt = design_runtime("compaction-running", (800.0, 620.0));
    let key = "compaction-0-opencode-compact";
    assert_eq!(
        element(&rt, key).label.as_deref(),
        Some("Compacting context…")
    );
    assert!(!element(&rt, key).focusable);
    let summary = "## Context\n\nKeep **drafts** intact.\n\n- Review the tool layout.";
    rt.update(|s, _| {
        s.live.as_mut().unwrap().app.sessions[0].conversation.apply(
            Provider::OpenCode,
            "session.compaction.delta",
            &serde_json::json!({"inputID":"compact","text":summary,"_created":100}),
        )
    });
    advance(&mut rt, 0.3);
    click(&mut rt, key);
    assert_eq!(element(&rt, key).expanded, Some(true));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    click(&mut rt, "copy-compaction-0-opencode-compact");
    assert_eq!(clipboard.text().unwrap().as_str(), summary);
    rt.update(|s, _| {
        let c = &mut s.live.as_mut().unwrap().app.sessions[0].conversation;
        c.apply(Provider::OpenCode, "session.compaction.ended", &serde_json::json!({"inputID":"compact","_created":200}));
        c.history(Provider::OpenCode, &[serde_json::json!({"id":"compact","type":"compaction","summary":"","status":"completed"})], false);
        c.working = false;
    });
    advance(&mut rt, 0.5);
    assert_eq!(
        element(&rt, key).label.as_deref(),
        Some("Context compacted")
    );
    assert_eq!(element(&rt, key).expanded, Some(true));
    assert_eq!(
        rt.elements()
            .iter()
            .filter(|e| e.key.as_deref() == Some(key) && !e.exiting)
            .count(),
        1
    );
    assert!(
        !rt.elements().iter().any(|e| e.path.contains(key)
            && e.key.as_deref() == Some("shimmer-band")
            && !e.exiting)
    );
    click(&mut rt, key);
    assert!(rt.find("copy-compaction-0-opencode-compact").is_none());
    for (scene, title) in [
        ("compaction-empty", "Context compacted"),
        ("compaction-error", "Context compaction failed"),
        ("compaction-interrupted", "Context compaction interrupted"),
    ] {
        let rt = design_runtime(scene, (800.0, 620.0));
        let milestone = element(&rt, key);
        assert_eq!(milestone.label.as_deref(), Some(title));
        assert!(!milestone.focusable);
        assert!(
            milestone
                .rect
                .iter()
                .zip(milestone.visible)
                .all(|(a, b)| (*a - b).abs() < 1.0)
        );
    }
    let mut expanded = design_runtime("compaction", (800.0, 620.0));
    let before = element(&expanded, key).rect[1];
    click(&mut expanded, key);
    advance(&mut expanded, 0.8);
    let header = element(&expanded, key);
    assert!(
        (header.rect[1] - before).abs() < 1.0,
        "before={before}, after={:?}, scroll={:?}",
        header.rect,
        header.scrolled
    );
    assert!((header.rect[3] - header.visible[3]).abs() < 1.0);
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
fn inline_patches_fit_short_changes_and_shrink_when_files_collapse() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        for scene in ["patch-short", "patch-long", "patch-multi"] {
            let mut rt = design_runtime(scene, size);
            advance(&mut rt, 0.6);
            let preview = "patch-preview-0-opencode-tool-patch";
            let expand = "expand-patch-0-opencode-tool-patch";
            let compact = element(&rt, preview);
            assert!(compact.rect[3] <= 206.0);
            assert!(
                (compact.rect[3] - compact.visible[3]).abs() < 1.0,
                "{scene} at {size:?}: {compact:?}"
            );
            if scene == "patch-short" {
                assert!(rt.find(expand).is_none());
                let last = element(&rt, "df0.n1");
                assert!((last.visible[3] - last.rect[3]).abs() < 1.0);
                assert!(
                    (compact.rect[1] + compact.rect[3] - last.rect[1] - last.rect[3] - 1.0).abs()
                        < 1.0
                );
            } else {
                click(&mut rt, expand);
                advance(&mut rt, 0.8);
                let expanded = element(&rt, preview);
                assert!(expanded.rect[3] > compact.rect[3]);
                assert!(expanded.rect[3] <= (size.1 * 0.5).min(480.0) + 2.0);
                assert!(element(&rt, expand).visible[3] >= 30.0);
                click(&mut rt, expand);
                advance(&mut rt, 0.8);
                assert!((element(&rt, preview).rect[3] - compact.rect[3]).abs() < 1.0);
            }
            let owner = (0, "opencode-tool-patch".to_owned());
            let files = rt.state.live.as_ref().unwrap().patches[&owner]
                .diff
                .files()
                .len();
            for _ in 0..files {
                let toggle = rt
                    .elements()
                    .into_iter()
                    .find(|e| {
                        !e.exiting
                            && e.path.contains(preview)
                            && e.label.as_deref() == Some("Collapse file")
                    })
                    .unwrap();
                click(&mut rt, &toggle.path);
                advance(&mut rt, 0.8);
            }
            assert!(rt.find(expand).is_none());
            assert!(element(&rt, preview).rect[3] < compact.rect[3]);
            assert!((0..files).all(|f| {
                rt.state.live.as_ref().unwrap().patches[&owner]
                    .diff
                    .collapsed(f)
            }));
            let toggle = rt
                .elements()
                .into_iter()
                .find(|e| {
                    !e.exiting
                        && e.path.contains(preview)
                        && e.label.as_deref() == Some("Expand file")
                })
                .unwrap();
            click(&mut rt, &toggle.path);
            assert!(
                !rt.state.live.as_ref().unwrap().patches[&owner]
                    .diff
                    .collapsed(0)
            );
        }
    }
}

#[test]
fn inline_patch_navigation_copy_and_selection_survive_theme_changes() {
    use rune_kit::diff::{LineRange, LineRef, Side};
    let mut rt = design_runtime("patch-multi", (1280.0, 1000.0));
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    let owner = (0, "opencode-tool-patch".to_owned());
    let view = format!("tool-{}", crate::code_view::source_key("0", &owner.1));
    let draft = rt.state.live.as_ref().unwrap().drafts[0].text.clone();
    click(&mut rt, &view);
    rt.update(|s, _| {
        s.live
            .as_mut()
            .unwrap()
            .patches
            .get_mut(&owner)
            .unwrap()
            .diff
            .set_selection(Some(LineRange::line(1, LineRef::new(Side::New, 1))));
    });
    key(&mut rt, Key::Text("]".into()), Mods::default());
    advance(&mut rt, 0.3);
    let selection = rt.state.live.as_ref().unwrap().patches[&owner]
        .diff
        .selection()
        .unwrap();
    assert_eq!(selection.file, 1);
    assert!(element(&rt, "df1").scrolled[1] > 0.0);
    for (family, _) in crate::theme::FAMILIES {
        for dark in [false, true] {
            rt.update(|s, _| {
                let p = &mut s.live.as_mut().unwrap().app.preferences;
                p.theme = if dark { "dark" } else { "light" }.into();
                p.theme_family = family.into();
                p.accent_color = "theme".into();
            });
            advance(&mut rt, 0.8);
            let palette = crate::theme::appearance(dark, family, "theme");
            assert_eq!(element(&rt, &view).fill, palette.c.bg_base);
            let header = rt
                .elements()
                .into_iter()
                .find(|e| {
                    !e.exiting
                        && e.path.contains(&view)
                        && e.role == "heading"
                        && e.visible[3] > 0.0
                })
                .unwrap();
            assert_eq!(header.fill, palette.c.bg_base);
            assert_eq!(
                rt.state.live.as_ref().unwrap().patches[&owner]
                    .diff
                    .selection(),
                Some(selection)
            );
        }
    }
    key(
        &mut rt,
        Key::Command('c'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert!(clipboard.text().unwrap().contains("MAX_HEIGHT"));
    key(&mut rt, Key::Escape, Mods::default());
    assert!(
        rt.state.live.as_ref().unwrap().patches[&owner]
            .diff
            .selection()
            .is_none()
    );
    let copy = rt
        .elements()
        .into_iter()
        .find(|e| {
            !e.exiting
                && e.path.contains(&view)
                && e.label.as_deref() == Some("Copy patch")
                && e.visible[3] > 0.0
        })
        .unwrap();
    click(&mut rt, &copy.path);
    let patch = clipboard.text().unwrap();
    assert!(patch.contains("--- a/src/session.rs") && patch.contains("--- a/src/theme.rs"));
    click(&mut rt, "tool-details-0-opencode-tool-patch");
    assert!(rt.state.live.as_ref().unwrap().expanded.contains(&owner));
    click(&mut rt, "tool-0-opencode-tool-patch");
    assert!(
        rt.state
            .live
            .as_ref()
            .unwrap()
            .collapsed_patches
            .contains(&owner)
    );
    assert!(rt.elements().iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.contains("src/session.rs, src/composer.rs, src/theme.rs"))
    }));
    assert_eq!(rt.state.live.as_ref().unwrap().drafts[0].text, draft);
    click(&mut rt, "tool-0-opencode-tool-patch");
    advance(&mut rt, 0.4);
    rt.update(|_, tx| tx.scroll_to_y(format!("{view}/scroll"), 0.0));
    advance(&mut rt, 0.2);
    click(&mut rt, "open-patch-file-0-opencode-tool-patch-0");
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.services.checkouts["/projects/caelum"]
            .selected_file
            .as_deref(),
        Some("src/session.rs")
    );
    assert!(rt.state.inspector.is_open());
}

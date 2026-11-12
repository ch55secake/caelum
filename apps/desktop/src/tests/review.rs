mod overview;

use super::*;

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
        diff.visible[3] >= 240.0,
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
fn review_refresh_and_tab_navigation_preserve_exact_patch_reading_position() {
    let mut state = crate::evaluation::workspace();
    let l = state.live.as_mut().unwrap();
    l.evaluation = false;
    l.fixture = true;
    crate::panels::open_review(
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

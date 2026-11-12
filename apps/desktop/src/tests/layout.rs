use super::*;

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

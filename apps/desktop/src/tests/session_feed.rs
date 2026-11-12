use super::*;
use caelum_core::application::SessionScope;

#[test]
fn feed_filters_and_scope_keep_session_identity_drafts_and_saved_preferences() {
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("session-feed", size);
        fill(&mut rt, "live-draft-0", "Keep café draft");
        reveal(&mut rt, "live-session-3");
        click(&mut rt, "live-session-3");
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .active_project
                .as_deref(),
            Some("/projects/rune")
        );
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Keep café draft"
        );
        click(&mut rt, "session-project-filter-trigger");
        click(&mut rt, "option1");
        assert!(rt.find("live-session-0").is_some());
        assert!(rt.find("live-session-3").is_none());
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(3));
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .preferences
                .session_scope,
            SessionScope::AllProjects
        );
        fill(&mut rt, "session-search", "no result");
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.text.as_deref() == Some("No matching sessions"))
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.2);
        assert!(rt.find("live-session-0").is_some());
        click(&mut rt, "nav-settings");
        rt.update(|_, tx| tx.scroll_into_view("session-scope-current"));
        advance(&mut rt, 0.4);
        click(&mut rt, "session-scope-current");
        let snapshot = rt.state.live.as_ref().unwrap().app.snapshot();
        assert_eq!(
            snapshot.preferences.session_scope,
            SessionScope::CurrentProject
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.2);
        assert!(rt.find("live-session-3").is_some());
        assert!(rt.find("live-session-0").is_none());
    }
}

#[test]
fn thousands_of_sessions_mount_only_a_window_and_keyboard_reaches_later_rows() {
    let mut rt = design_runtime("session-feed-many", (800.0, 620.0));
    let mounted = rt
        .elements()
        .iter()
        .filter(|e| {
            !e.exiting
                && e.key
                    .as_ref()
                    .is_some_and(|k| k.starts_with("live-session-"))
        })
        .count();
    assert!(mounted < 40, "mounted {mounted} session rows");
    let order = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .app
        .session_feed(None, "", false);
    reveal(&mut rt, &format!("live-session-{}", order[0]));
    click(&mut rt, &format!("live-session-{}", order[0]));
    for _ in 0..35 {
        key(&mut rt, Key::Down, Mods::default());
        advance(&mut rt, 0.05);
    }
    assert_eq!(
        rt.state.live.as_ref().unwrap().app.selected,
        Some(order[35])
    );
    assert!(element(&rt, &format!("live-session-{}", order[35])).visible[3] > 80.0);
    eprintln!("2,006 sessions at 800×620: {mounted} mounted rows; keyboard reached row 36");
}

#[test]
fn pull_request_rows_follow_the_session_filter_and_keep_repository_identity_and_focus() {
    let rune_url = "https://github.com/example/rune/pull/42";
    let website_url = "https://github.com/example/website/pull/42";
    let rune = crate::code_view::source_key(rune_url, "pull-request");
    let website = crate::code_view::source_key(website_url, "pull-request");
    for size in [(800.0, 620.0), (1280.0, 1000.0)] {
        let mut rt = design_runtime("session-feed", size);
        fill(&mut rt, "live-draft-0", "Preserve this café draft");
        assert!(rt.find(&rune).is_some());
        assert!(rt.find(&website).is_some());
        click(&mut rt, "session-project-filter-trigger");
        click(&mut rt, "option2");
        assert!(rt.find(&rune).is_some());
        assert!(rt.find(&website).is_none());
        assert_eq!(rt.state.live.as_ref().unwrap().app.selected, Some(0));
        assert_eq!(
            rt.state
                .live
                .as_ref()
                .unwrap()
                .app
                .active_project
                .as_deref(),
            Some("/projects/caelum")
        );
        fill(&mut rt, "session-search", "no session matches");
        assert!(
            rt.find(&rune).is_some(),
            "session search must not hide scoped pull requests"
        );
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.2);
        reveal(&mut rt, &rune);
        assert!(rt.accessibility_action(&rune, rune::accesskit::Action::Focus, None));
        secondary(&mut rt, &rune);
        key(&mut rt, Key::Escape, Mods::default());
        advance(&mut rt, 0.2);
        assert!(
            element(&rt, &rune).focused,
            "context menu returns to the URL-owned row"
        );
        click(&mut rt, "session-project-filter-trigger");
        click(&mut rt, "option3");
        assert!(rt.find(&rune).is_none());
        assert!(rt.find(&website).is_some());
        reveal(&mut rt, &website);
        click(&mut rt, &website);
        assert_eq!(
            rt.state.live.as_ref().unwrap().panels.review,
            Some(("/projects/website".into(), website_url.into()))
        );
        assert_eq!(
            rt.state.live.as_ref().unwrap().drafts[0].text,
            "Preserve this café draft"
        );
        click(&mut rt, "session-project-filter-trigger");
        click(&mut rt, "option0");
        assert!(rt.find(&rune).is_some());
        assert!(rt.find(&website).is_some());
        rt.update(|s, _| {
            s.live.as_mut().unwrap().app.preferences.session_scope = SessionScope::CurrentProject
        });
        advance(&mut rt, 0.2);
        assert!(rt.find(&rune).is_none());
        assert!(rt.find(&website).is_none());
    }
}

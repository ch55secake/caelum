use super::*;

#[test]
fn long_descriptions_leave_readiness_visible_and_checks_reachable_in_both_themes() {
    for dark in [false, true] {
        for size in [(800.0, 620.0), (1280.0, 1000.0)] {
            let app = spec();
            let mut state = crate::evaluation::design();
            crate::evaluation::scene(&mut state, "overview-long");
            let mut rt = Runtime::new(state, app.view, size, Env::new(dark, 1.0));
            rt.poll = app.poll;
            rt.on_key = app.on_key;
            advance(&mut rt, 0.5);
            assert!(element(&rt, "review-header").rect[3] < 170.0);
            assert!(element(&rt, "review-title").rect[3] <= 53.0);
            for key in [
                "review-approval",
                "review-check-summary",
                "review-branch",
                "merge-controls",
            ] {
                let e = element(&rt, key);
                assert!(
                    e.visible[3] >= e.rect[3] - 1.0,
                    "{size:?} {key} is clipped: {e:?}"
                );
            }
            let readiness = element(&rt, "review-merge-box");
            assert!(
                element(&rt, "review-description").rect[1] >= readiness.rect[1] + readiness.rect[3]
            );
            assert!(!element(&rt, "merge-pull-request").focusable);
            click(&mut rt, "review-checks-toggle");
            assert_eq!(element(&rt, "review-checks-toggle").expanded, Some(true));
            assert_eq!(
                element(&rt, "check-name-0").text.as_deref(),
                Some("Build native desktop / aarch64-apple-darwin")
            );
            reveal(&mut rt, "merge-controls");
            click(&mut rt, "review-tab-1");
            let diff = rt
                .elements()
                .into_iter()
                .find(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
                .unwrap();
            assert!(diff.visible[3] >= 240.0, "{size:?} code viewport: {diff:?}");
        }
    }
}

#[test]
fn header_identity_and_refresh_follow_the_review_while_back_restores_the_selected_draft() {
    use caelum_core::application::{Command, Provider, Scope, Task};

    let mut rt = design_runtime("overview", (800.0, 620.0));
    let target = (
        "/projects/caelum".to_owned(),
        "https://github.com/example/remote-repository/pull/42".to_owned(),
    );
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let previous = l.panels.review.take().unwrap();
        let mut review = l.app.services.reviews.remove(&previous).unwrap();
        let details = review.details.as_mut().unwrap();
        details.pull_request.url = target.1.clone();
        details.pull_request.number = 42;
        details.pull_request.is_draft = true;
        details.state = "CLOSED".into();
        l.app.services.reviews.insert(target.clone(), review);
        l.panels.review = Some(target.clone());
        l.app.projects.push(caelum_protocol::ProjectInfo {
            name: "Other".into(),
            path: "/projects/other".into(),
        });
        let index = l.app.new_session(Scope {
            project: "/projects/other".into(),
            provider: Provider::Codex,
        });
        l.app.sessions[index].draft = "Keep this other project's draft".into();
        l.sync_editors();
        l.app.services.revision += 1;
    });
    advance(&mut rt, 0.3);
    assert_eq!(
        element(&rt, "review-repository").text.as_deref(),
        Some("example/remote-repository")
    );
    assert_eq!(element(&rt, "review-number").text.as_deref(), Some("#42"));
    assert_eq!(element(&rt, "review-state").text.as_deref(), Some("Closed"));
    assert!(rt.find("merge-pull-request").is_none());
    rt.state.live.as_mut().unwrap().app.commands();
    click(&mut rt, "review-refresh");
    let l = rt.state.live.as_mut().unwrap();
    assert!(l.app.commands().iter().any(|(_, c)| matches!(c, Command::Task(Task::Details { project, url }) if (project, url) == (&target.0, &target.1))));
    let index = l.app.selected.unwrap();
    assert_eq!(l.app.active_project.as_deref(), Some("/projects/other"));
    // The header must remain usable beside an inspector at the minimum window size.
    rt.update(|s, _| s.inspector.set_open(true));
    advance(&mut rt, 0.5);
    for key in [
        "back-to-session",
        "review-repository",
        "review-number",
        "review-state",
        "review-github",
        "review-refresh",
        "review-title",
    ] {
        let e = element(&rt, key);
        assert!(
            e.visible[2] > 0.0
                && e.visible[2] >= e.rect[2] - 1.0
                && e.visible[3] >= e.rect[3] - 1.0,
            "{key} is clipped: {e:?}"
        );
    }
    click(&mut rt, "back-to-session");
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.app.selected, Some(index));
    assert_eq!(l.drafts[index].text, "Keep this other project's draft");
    assert!(element(&rt, &format!("live-draft-{index}")).focused);
}

#[test]
fn loading_and_failed_reviews_keep_navigation_and_retained_content_accessible() {
    for scene in ["review-loading", "review-error", "overview-stale"] {
        let mut rt = design_runtime(scene, (800.0, 620.0));
        assert!(element(&rt, "review-number").visible[2] > 0.0);
        assert!(element(&rt, "review-refresh").focusable);
        if scene == "overview-stale" {
            reveal(&mut rt, "review-branch");
            assert!(
                rt.elements()
                    .iter()
                    .any(|e| e.text.as_deref() == Some("Review request failed"))
            );
            click(&mut rt, "review-tab-1");
            assert!(
                rt.elements()
                    .iter()
                    .any(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
            );
        }
        click(&mut rt, "back-to-session");
        assert_eq!(
            rt.state.live.as_ref().unwrap().panels.screen,
            crate::panels::Screen::Conversation
        );
    }
}

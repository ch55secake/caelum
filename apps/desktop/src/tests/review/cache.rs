use super::*;
use caelum_core::application::{Command, Event, Reply, Task, TaskReply};

fn diff_offset(rt: &Runtime<Workspace>) -> f32 {
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
}

#[test]
fn cached_reviews_show_files_without_presenting_old_checks_as_current() {
    for dark in [false, true] {
        let app = spec();
        let mut state = crate::evaluation::design();
        crate::evaluation::scene(&mut state, "overview-cached");
        let mut rt = Runtime::new(state, app.view, (800.0, 620.0), Env::new(dark, 1.0));
        rt.poll = app.poll;
        advance(&mut rt, 0.5);
        assert_eq!(
            element(&rt, "review-cached").text.as_deref(),
            Some("Cached · refreshing")
        );
        assert!(element(&rt, "review-cached-readiness").visible[3] > 0.0);
        for key in [
            "merge-pull-request",
            "set-auto-merge",
            "review-check-summary",
        ] {
            assert!(rt.find(key).is_none(), "{key}");
        }
        // The extra cache label must fit with the inspector at minimum size.
        rt.update(|s, _| s.inspector.set_open(true));
        advance(&mut rt, 0.3);
        for key in ["review-cached", "review-refresh", "review-github"] {
            let e = element(&rt, key);
            assert!(e.visible[2] >= e.rect[2] - 1.0, "{key}: {e:?}");
        }
        rt.update(|s, _| s.inspector.set_open(false));
        click(&mut rt, "review-tab-1");
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
        );
        rt.update(|_, tx| tx.scroll_to_y("diff/scroll", 1000.0));
        advance(&mut rt, 0.3);
        let before = diff_offset(&rt);
        assert!(before > 900.0);
        // Refresh metadata in place: unchanged commits keep the mounted diff.
        rt.update(|s, _| {
            let l = s.live.as_mut().unwrap();
            let owner = l.panels.review.clone().unwrap();
            let details = l.app.services.reviews[&owner].details.clone().unwrap();
            let request = l
                .app
                .commands()
                .into_iter()
                .find(|(_, c)| matches!(c, Command::Task(Task::Details { .. })))
                .unwrap()
                .0;
            l.apply(Event::Completed(
                request,
                Ok(Reply::Task(TaskReply::Details(Box::new(details)))),
            ));
        });
        advance(&mut rt, 0.3);
        assert!((diff_offset(&rt) - before).abs() < 1.0);
        assert!(rt.find("review-cached").is_none());
        click(&mut rt, "review-tab-0");
        assert!(element(&rt, "merge-pull-request").focusable);
        assert!(rt.find("review-check-summary").is_some());
    }
}

#[test]
fn failed_background_refresh_leaves_a_quiet_cached_review_and_readable_diff() {
    let mut rt = design_runtime("overview-cached", (800.0, 620.0));
    rt.update(|s, _| {
        let l = s.live.as_mut().unwrap();
        let request = l
            .app
            .commands()
            .into_iter()
            .find(|(_, c)| matches!(c, Command::Task(Task::Details { .. })))
            .unwrap()
            .0;
        l.apply(Event::Completed(
            request,
            Err("GitHub is unavailable".into()),
        ));
    });
    advance(&mut rt, 0.3);
    assert_eq!(
        element(&rt, "review-cached").text.as_deref(),
        Some("Cached")
    );
    assert!(rt.find("merge-pull-request").is_none());
    click(&mut rt, "review-tab-1");
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.path.ends_with("/diff/scroll") && !e.exiting)
    );
    assert!(element(&rt, "review-refresh").focusable);
}

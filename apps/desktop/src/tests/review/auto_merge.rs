use super::*;
use caelum_core::application::{Command, Event, Reply, Task, TaskReply};
use caelum_protocol::{GitHubAutoMergeRequest, GitHubAutoMergeResult, GitHubMergeMethod};

#[test]
fn auto_merge_controls_fit_both_themes_and_compact_reviews_with_the_inspector() {
    for dark in [false, true] {
        for size in [(800.0, 620.0), (1280.0, 1000.0)] {
            for enabled in [false, true] {
                let app = spec();
                let mut state = crate::evaluation::design();
                crate::evaluation::scene(
                    &mut state,
                    if enabled {
                        "merge-auto-enabled"
                    } else {
                        "merge-blocked"
                    },
                );
                let mut rt = Runtime::new(state, app.view, size, Env::new(dark, 1.0));
                rt.poll = app.poll;
                advance(&mut rt, 0.5);
                for inspector in [false, true] {
                    rt.update(|s, _| s.inspector.set_open(inspector));
                    advance(&mut rt, 0.5);
                    reveal(&mut rt, "set-auto-merge");
                    let button = element(&rt, "set-auto-merge");
                    assert!(button.focusable);
                    assert!(button.visible[2] >= button.rect[2] - 1.0, "{button:?}");
                    assert_eq!(
                        button.label.as_deref(),
                        Some(if enabled {
                            "Disable auto-merge"
                        } else {
                            "Enable auto-merge"
                        })
                    );
                    assert!(!element(&rt, "merge-pull-request").focusable);
                    if enabled {
                        assert!(
                            element(&rt, "auto-merge-status")
                                .text
                                .unwrap()
                                .contains("squash")
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn auto_merge_uses_the_selected_method_locks_mutations_and_keeps_failures_retryable() {
    for enabled in [false, true] {
        let mut rt = design_runtime(
            if enabled {
                "merge-auto-enabled"
            } else {
                "merge-behind"
            },
            (800.0, 620.0),
        );
        click(&mut rt, "merge-method-trigger");
        key(&mut rt, Key::End, Mods::default());
        key(&mut rt, Key::Enter, Mods::default());
        advance(&mut rt, 0.2);
        click(&mut rt, "set-auto-merge");
        let (request, task) = take_mutation(&mut rt);
        let Task::SetAutoMerge {
            project,
            url,
            head,
            method,
        } = task
        else {
            panic!("wrong task")
        };
        let original = (project, url);
        assert_eq!(original.0, "/projects/caelum");
        assert_eq!(original.1, "https://github.com/ch55secake/caelum/pull/95");
        assert_eq!(head, "a".repeat(40));
        assert_eq!(method, (!enabled).then_some(GitHubMergeMethod::Rebase));
        for control in [
            "set-auto-merge",
            "merge-method-trigger",
            "merge-pull-request",
        ] {
            assert!(!element(&rt, control).focusable, "{control}");
        }
        if !enabled {
            assert!(!element(&rt, "update-review-branch").focusable);
        }
        click(&mut rt, "set-auto-merge");
        assert!(
            !rt.state
                .live
                .as_mut()
                .unwrap()
                .app
                .commands()
                .iter()
                .any(|(_, command)| matches!(command, Command::Task(Task::SetAutoMerge { .. })))
        );
        rt.update(|s, _| {
            s.live
                .as_mut()
                .unwrap()
                .apply(Event::Completed(request, Err("Permission denied".into())))
        });
        advance(&mut rt, 0.2);
        assert!(element(&rt, "set-auto-merge").focusable);
        assert_eq!(
            element(&rt, "set-auto-merge").label.as_deref(),
            Some(if enabled {
                "Disable auto-merge"
            } else {
                "Enable auto-merge"
            })
        );
        assert!(
            rt.elements()
                .iter()
                .any(|element| element.text.as_deref() == Some("Permission denied"))
        );
        reveal(&mut rt, "set-auto-merge");
        click(&mut rt, "set-auto-merge");
        let (retry, _) = take_mutation(&mut rt);
        rt.update(|s, _| {
            let l = s.live.as_mut().unwrap();
            l.apply(Event::Completed(
                retry,
                Ok(Reply::Task(TaskReply::AutoMerge(GitHubAutoMergeResult {
                    head_ref_oid: head.clone(),
                    state: "OPEN".into(),
                    auto_merge_request: method
                        .map(|merge_method| GitHubAutoMergeRequest { merge_method }),
                }))),
            ));
        });
        advance(&mut rt, 0.2);
        assert_eq!(
            element(&rt, "set-auto-merge").label.as_deref(),
            Some(if enabled {
                "Enable auto-merge"
            } else {
                "Disable auto-merge"
            })
        );
        if !enabled {
            assert!(
                element(&rt, "auto-merge-status")
                    .text
                    .unwrap()
                    .contains("rebase")
            );
        }
        assert!(rt.state.live.as_mut().unwrap().app.commands().iter().any(|(_, command)| matches!(command, Command::Task(Task::Details { project, url }) if (project,url) == (&original.0,&original.1))));
    }
}

#[test]
fn changing_reviews_during_auto_merge_keeps_the_original_owner() {
    let mut rt = design_runtime("merge-blocked", (800.0, 620.0));
    click(&mut rt, "set-auto-merge");
    let (request, _) = take_mutation(&mut rt);
    let original = rt
        .state
        .live
        .as_ref()
        .unwrap()
        .panels
        .review
        .clone()
        .unwrap();
    rt.update(|s, _| {
        crate::panels::open_review(
            s,
            "/other".into(),
            "https://github.com/example/other/pull/44".into(),
        );
        let l = s.live.as_mut().unwrap();
        l.app.select(1);
        l.apply(Event::Completed(
            request,
            Ok(Reply::Task(TaskReply::AutoMerge(GitHubAutoMergeResult {
                head_ref_oid: "a".repeat(40),
                state: "OPEN".into(),
                auto_merge_request: Some(GitHubAutoMergeRequest {
                    merge_method: GitHubMergeMethod::Squash,
                }),
            }))),
        ));
    });
    advance(&mut rt, 0.2);
    let l = rt.state.live.as_ref().unwrap();
    assert_eq!(l.app.selected, Some(1));
    assert_eq!(l.panels.review.as_ref().unwrap().0, "/other");
    assert!(
        l.app.services.reviews[&original]
            .details
            .as_ref()
            .unwrap()
            .auto_merge_request
            .is_some()
    );
}

fn take_mutation(rt: &mut Runtime<Workspace>) -> (u64, Task) {
    rt.state
        .live
        .as_mut()
        .unwrap()
        .app
        .commands()
        .into_iter()
        .find_map(|(request, command)| {
            if let Command::Task(task @ Task::SetAutoMerge { .. }) = command {
                Some((request, task))
            } else {
                None
            }
        })
        .expect("missing auto-merge task")
}

use super::*;

#[test]
fn search_and_review_pages_reject_late_results_from_old_targets() {
    let mut app = Workspace::default();
    for query in ["old", "new"] {
        app.task(Task::Search {
            project: "/a".into(),
            query: query.into(),
            mode: caelum_protocol::ProjectSearchMode::Files,
        });
    }
    let commands = app.commands();
    let result = || {
        Reply::Task(TaskReply::Search(caelum_protocol::ProjectSearchResults {
            matches: vec![],
            truncated: false,
            skipped: 0,
        }))
    };
    app.apply(Event::Completed(commands[1].0, Ok(result())));
    app.apply(Event::Completed(commands[0].0, Ok(result())));
    assert_eq!(app.services.checkouts["/a"].search_query, "new");
    let key = ("/a".into(), "https://github.com/a/b/pull/1".into());
    let details: caelum_protocol::GitHubPullRequestDetails = serde_json::from_value(json!({"number":1,"title":"PR","url":key.1,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"new","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    app.services.reviews.entry(key.clone()).or_default().details = Some(details);
    app.task(Task::ReviewFiles {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "old".into(),
        page: 1,
    });
    let (id, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Task(TaskReply::ReviewFiles(
            caelum_protocol::GitHubPullRequestFiles {
                files: vec![],
                base_oid: "base".into(),
                next_page: Some(2),
            },
        ))),
    ));
    assert!(!app.services.reviews[&key].files_loaded);
}

fn discover_pull_request(
    app: &mut Workspace,
    project: &str,
    branch: &str,
    pr: &caelum_protocol::GitHubPullRequest,
) {
    app.task(Task::Discover {
        project: project.into(),
        branch: branch.into(),
    });
    let (id, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(
        id,
        Ok(Reply::Task(TaskReply::Discovered(Some(pr.clone())))),
    ));
}

#[test]
fn merged_pull_requests_leave_session_associations_and_stale_discovery_cannot_restore_them() {
    let mut app = Workspace::default();
    let pr = caelum_protocol::GitHubPullRequest {
        number: 42,
        title: "Earlier work".into(),
        url: "https://github.com/example/repo/pull/42".into(),
        is_draft: false,
        base_ref_name: String::new(),
    };
    for project in ["/a", "/b"] {
        discover_pull_request(&mut app, project, "feature", &pr);
    }
    for project in ["/a", "/b"] {
        let associated = &app.services.checkouts[project].pull_requests;
        assert_eq!(associated.len(), 1);
        assert_eq!(associated[0], pr);
    }

    app.task(Task::Discover {
        project: "/b".into(),
        branch: "feature".into(),
    });
    let stale_discovery = app.commands().pop().unwrap().0;
    app.task(Task::Merge {
        project: "/a".into(),
        url: pr.url.clone(),
        head: "0123456789abcdef0123456789abcdef01234567".into(),
        method: caelum_protocol::GitHubMergeMethod::Squash,
    });
    let (merge, _) = app
        .commands()
        .into_iter()
        .find(|(_, command)| matches!(command, Command::Task(Task::Merge { .. })))
        .unwrap();
    app.apply(Event::Completed(merge, Ok(Reply::Task(TaskReply::Done))));
    for checkout in app.services.checkouts.values() {
        assert!(checkout.pull_requests.is_empty());
    }

    app.apply(Event::Completed(
        stale_discovery,
        Ok(Reply::Task(TaskReply::Discovered(Some(pr.clone())))),
    ));
    for checkout in app.services.checkouts.values() {
        assert!(checkout.pull_requests.is_empty());
    }
}

#[test]
fn refreshed_merged_pull_request_is_removed_from_session_associations() {
    let mut app = Workspace::default();
    let pr = caelum_protocol::GitHubPullRequest {
        number: 43,
        title: "Merged elsewhere".into(),
        url: "https://github.com/example/repo/pull/43".into(),
        is_draft: false,
        base_ref_name: String::new(),
    };
    discover_pull_request(&mut app, "/a", "feature", &pr);
    app.task(Task::Details {
        project: "/a".into(),
        url: pr.url.clone(),
    });
    let (request, _) = app.commands().pop().unwrap();
    let details = serde_json::from_value(json!({
        "number": 43,
        "title": "Merged elsewhere",
        "url": pr.url,
        "isDraft": false,
        "baseRefName": "main",
        "body": "",
        "state": "MERGED",
        "author": null,
        "headRefName": "feature",
        "headRefOid": "0123456789abcdef0123456789abcdef01234567",
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "additions": 1,
        "deletions": 0,
        "changedFiles": 1,
        "reviewDecision": null,
        "statusCheckRollup": []
    }))
    .unwrap();
    app.apply(Event::Completed(
        request,
        Ok(Reply::Task(TaskReply::Details(details))),
    ));
    assert!(app.services.checkouts["/a"].pull_requests.is_empty());
}

#[test]
fn same_head_refresh_rejects_old_pages_without_unlocking_an_active_mutation() {
    let mut app = Workspace::default();
    let key = ("/a".to_owned(), "https://github.com/a/b/pull/1".to_owned());
    let details = serde_json::from_value(json!({"number":1,"title":"PR","url":key.1,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"head","mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":1,"deletions":0,"changedFiles":1,"reviewDecision":null,"statusCheckRollup":[]})).unwrap();
    app.services.reviews.entry(key.clone()).or_default().details = Some(details);
    let files = Task::ReviewFiles {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "head".into(),
        page: 1,
    };
    let mutation = Task::UpdateBranch {
        project: key.0.clone(),
        url: key.1.clone(),
        head: "head".into(),
    };
    app.task(files.clone());
    let (old, _) = app.commands().pop().unwrap();
    app.task(mutation.clone());
    let (update, _) = app.commands().pop().unwrap();
    app.services.refresh_review(&key.0, &key.1);
    assert!(app.services.busy(&mutation));
    app.task(files.clone());
    let (new, _) = app.commands().pop().unwrap();
    app.apply(Event::Completed(old, Err("stale failure".into())));
    assert!(app.services.busy(&files));
    assert!(app.services.reviews[&key].error.is_none());
    app.apply(Event::Completed(
        new,
        Ok(Reply::Task(TaskReply::ReviewFiles(
            caelum_protocol::GitHubPullRequestFiles {
                files: vec![],
                base_oid: "base".into(),
                next_page: None,
            },
        ))),
    ));
    assert!(app.services.reviews[&key].files_loaded);
    app.apply(Event::Completed(update, Ok(Reply::Task(TaskReply::Done))));
    assert!(!app.services.busy(&mutation));
    assert!(app.services.reviews[&key].notice.is_some());
}

#[test]
fn discovery_refresh_replaces_and_removes_only_its_branch_and_preserves_cache_on_failure() {
    let mut app = Workspace::default();
    let pr = caelum_protocol::GitHubPullRequest {
        number: 42,
        title: "Initial title".into(),
        url: "https://github.com/example/alpha/pull/42".into(),
        is_draft: false,
        base_ref_name: String::new(),
    };
    let other = caelum_protocol::GitHubPullRequest {
        url: "https://github.com/example/beta/pull/42".into(),
        ..pr.clone()
    };
    discover_pull_request(&mut app, "/alpha", "feature", &pr);
    discover_pull_request(&mut app, "/beta", "feature", &other);
    let updated = caelum_protocol::GitHubPullRequest {
        title: "Updated title".into(),
        ..pr.clone()
    };
    discover_pull_request(&mut app, "/alpha", "feature", &updated);
    assert_eq!(
        app.services.checkouts["/alpha"].pull_requests.as_slice(),
        std::slice::from_ref(&updated)
    );

    for reply in [
        Err("Temporary network failure".into()),
        Ok(Reply::Task(TaskReply::Discovered(None))),
    ] {
        app.task(Task::Discover {
            project: "/alpha".into(),
            branch: "feature".into(),
        });
        let request = app.commands().pop().unwrap().0;
        let failed = reply.is_err();
        app.apply(Event::Completed(request, reply));
        assert_eq!(
            app.services.checkouts["/alpha"].pull_requests.len(),
            usize::from(failed)
        );
        assert_eq!(
            app.services.checkouts["/beta"].pull_requests.as_slice(),
            std::slice::from_ref(&other)
        );
    }
    discover_pull_request(&mut app, "/alpha", "feature", &pr);
    let replacement = caelum_protocol::GitHubPullRequest {
        number: 43,
        url: "https://github.com/example/alpha/pull/43".into(),
        ..updated
    };
    discover_pull_request(&mut app, "/alpha", "feature", &replacement);
    assert_eq!(
        app.services.checkouts["/alpha"].pull_requests,
        [replacement]
    );
}

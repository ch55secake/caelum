use super::*;
use crate::github::CachedReview;
use caelum_protocol::{GitHubPullRequestDetails, GitHubPullRequestFiles};

const URL: &str = "https://git.example.com/team/repo/pull/42";
fn details(head: &str, base: &str) -> GitHubPullRequestDetails {
    serde_json::from_value(json!({
        "number":42, "title":"Review", "url":URL, "isDraft":false, "body":"Cached overview",
        "state":"OPEN", "baseRefName":"main", "baseRefOid":base, "headRefName":"feature",
        "headRefOid":head, "mergeable":"MERGEABLE", "mergeStateStatus":"CLEAN",
        "additions":1, "deletions":0, "changedFiles":60
    }))
    .unwrap()
}
fn cached() -> Box<CachedReview> {
    Box::new(CachedReview {
        details: details("head", "base"),
        files: Some(GitHubPullRequestFiles {
            files: vec![serde_json::from_value(json!({"path":"src/first.rs", "status":"added", "additions":1, "deletions":0, "patch":"@@ -0,0 +1 @@\n+new"})).unwrap()],
            base_oid: "base".into(), next_page: Some(2),
        }),
    })
}
fn open(app: &mut Workspace, project: &str) -> (u64, u64) {
    app.open_review(project.into(), URL.into());
    let commands = app.commands();
    let cache = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::ReviewCache { .. })))
        .unwrap()
        .0;
    let live = commands
        .iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::Details { .. })))
        .unwrap()
        .0;
    (cache, live)
}
fn finish(app: &mut Workspace, id: u64, reply: TaskReply) {
    app.apply(Event::Completed(id, Ok(Reply::Task(reply))));
}

#[test]
fn cached_first_open_keeps_files_and_pagination_when_fresh_commits_match() {
    let mut app = Workspace::default();
    let (cache, live) = open(&mut app, "/worktree");
    finish(&mut app, cache, TaskReply::ReviewCache(Some(cached())));
    let key = ("/worktree".into(), URL.into());
    let review = &app.services.reviews[&key];
    assert!(review.cached() && review.files_loaded);
    assert_eq!(review.files.len(), 1);
    assert!(
        app.commands().is_empty(),
        "cache cannot launch stale followups"
    );
    finish(
        &mut app,
        live,
        TaskReply::Details(details("head", "base").into()),
    );
    assert!(!app.services.reviews[&key].cached());
    assert_eq!(app.services.reviews[&key].next_page, Some(2));
    let next = app.commands();
    assert_eq!(next.len(), 3, "only feedback needs fetching");
    assert!(next
        .iter()
        .all(|(_, c)| matches!(c, Command::Task(Task::Feedback { .. }))));
    app.task(Task::ReviewFiles {
        project: key.0.clone(),
        url: URL.into(),
        head: "head".into(),
        base: "base".into(),
        page: 2,
    });
    let id = app.commands().pop().unwrap().0;
    finish(
        &mut app,
        id,
        TaskReply::ReviewFiles(GitHubPullRequestFiles {
            files: vec![],
            base_oid: "base".into(),
            next_page: None,
        }),
    );
    assert_eq!(app.services.reviews[&key].files.len(), 1);
    assert_eq!(app.services.reviews[&key].next_page, None);
}

#[test]
fn cache_races_base_only_changes_and_head_changes_preserve_live_ownership() {
    for cache_first in [false, true] {
        for (head, base) in [("head", "base"), ("head", "new-base"), ("new-head", "base")] {
            let mut app = Workspace::default();
            let (cache, live) = open(&mut app, "/a");
            if cache_first {
                finish(&mut app, cache, TaskReply::ReviewCache(Some(cached())));
            }
            let mut fresh = details(head, base);
            fresh.body = "Live overview".into();
            finish(&mut app, live, TaskReply::Details(fresh.clone().into()));
            if !cache_first {
                finish(&mut app, cache, TaskReply::ReviewCache(Some(cached())));
            }
            let review = &app.services.reviews[&("/a".into(), URL.into())];
            assert_eq!(review.details.as_ref(), Some(&fresh));
            assert!(!review.cached());
            assert_eq!(
                review.files_loaded,
                cache_first && head == "head" && base == "base"
            );
            if !review.files_loaded {
                assert!(review.files.is_empty());
                assert!(app.commands().iter().any(|(_, c)| matches!(c,
                    Command::Task(Task::ReviewFiles { head: h, base: b, page: 1, .. }) if h == head && b == base)));
            }
        }
    }
}

#[test]
fn failed_refresh_keeps_cached_content_without_admitting_mutations_or_stale_pages() {
    let mut app = Workspace::default();
    let (cache, live) = open(&mut app, "/a");
    app.apply(Event::Completed(live, Err("Offline".into())));
    finish(&mut app, cache, TaskReply::ReviewCache(Some(cached())));
    for task in [
        Task::Merge {
            project: "/a".into(),
            url: URL.into(),
            head: "head".into(),
            method: caelum_protocol::GitHubMergeMethod::Squash,
        },
        Task::UpdateBranch {
            project: "/a".into(),
            url: URL.into(),
            head: "head".into(),
        },
        Task::SetAutoMerge {
            project: "/a".into(),
            url: URL.into(),
            head: "head".into(),
            method: None,
        },
        Task::ReviewFiles {
            project: "/a".into(),
            url: URL.into(),
            head: "head".into(),
            base: "base".into(),
            page: 2,
        },
    ] {
        app.task(task);
    }
    assert!(app.commands().is_empty());
    let review = &app.services.reviews[&("/a".into(), URL.into())];
    assert!(review.cached() && review.files_loaded);
    assert_eq!(review.error.as_deref(), Some("Offline"));
}

#[test]
fn manual_refresh_rejects_late_cache_and_old_base_pages() {
    let mut app = Workspace::default();
    let (cache, live) = open(&mut app, "/a");
    app.services.refresh_review("/a", URL);
    finish(&mut app, cache, TaskReply::ReviewCache(Some(cached())));
    finish(
        &mut app,
        live,
        TaskReply::Details(details("head", "base").into()),
    );
    assert!(app.services.reviews[&("/a".into(), URL.into())]
        .details
        .is_none());
    app.task(Task::Details {
        project: "/a".into(),
        url: URL.into(),
    });
    let live = app.commands().pop().unwrap().0;
    finish(
        &mut app,
        live,
        TaskReply::Details(details("head", "base").into()),
    );
    let old_page = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::ReviewFiles { .. })))
        .unwrap()
        .0;
    app.task(Task::Details {
        project: "/a".into(),
        url: URL.into(),
    });
    let live = app.commands().pop().unwrap().0;
    finish(
        &mut app,
        live,
        TaskReply::Details(details("head", "new-base").into()),
    );
    let new_page = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Task(Task::ReviewFiles { .. })))
        .unwrap()
        .0;
    finish(
        &mut app,
        old_page,
        TaskReply::ReviewFiles(cached().files.unwrap()),
    );
    assert!(!app.services.reviews[&("/a".into(), URL.into())].files_loaded);
    finish(
        &mut app,
        new_page,
        TaskReply::ReviewFiles(GitHubPullRequestFiles {
            files: vec![],
            base_oid: "new-base".into(),
            next_page: None,
        }),
    );
    assert!(app.services.reviews[&("/a".into(), URL.into())].files_loaded);
}

#[test]
fn cache_misses_and_other_worktrees_do_not_interfere_with_fresh_loading() {
    let mut app = Workspace::default();
    let (a_cache, a_live) = open(&mut app, "/a");
    let (b_cache, b_live) = open(&mut app, "/b");
    finish(&mut app, a_cache, TaskReply::ReviewCache(None));
    finish(&mut app, b_cache, TaskReply::ReviewCache(Some(cached())));
    finish(
        &mut app,
        a_live,
        TaskReply::Details(details("new-head", "base").into()),
    );
    assert!(app.services.reviews[&("/b".into(), URL.into())].cached());
    finish(
        &mut app,
        b_live,
        TaskReply::Details(details("head", "base").into()),
    );
    assert_eq!(
        app.services.reviews[&("/a".into(), URL.into())].head(),
        Some("new-head")
    );
    assert_eq!(
        app.services.reviews[&("/b".into(), URL.into())].head(),
        Some("head")
    );
}

#[test]
fn review_reads_do_not_wait_for_unrelated_draft_persistence() {
    let mut app = Workspace::default();
    app.enable_store("/unused/workspace.json".into());
    let load = app.commands().pop().unwrap().0;
    app.apply(Event::Completed(load, Ok(Reply::Snapshot(None))));
    app.preferences.theme = "dark".into();
    let save = app
        .commands()
        .into_iter()
        .find(|(_, c)| matches!(c, Command::Save(..)))
        .unwrap()
        .0;
    open(&mut app, "/a");
    app.apply(Event::Completed(save, Err("disk full".into())));
    open(&mut app, "/b");
}

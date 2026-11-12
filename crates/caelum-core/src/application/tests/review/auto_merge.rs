use super::*;
use caelum_protocol::{
    GitHubAutoMergeRequest, GitHubAutoMergeResult, GitHubMergeMethod, GitHubPullRequestDetails,
};

const URL: &str = "https://github.com/example/repo/pull/44";

fn fixture() -> (Workspace, (String, String)) {
    let mut app = Workspace::default();
    let key = ("/project".to_owned(), URL.to_owned());
    let details: GitHubPullRequestDetails = serde_json::from_value(json!({
        "number":44,"title":"Review","url":URL,"isDraft":false,"body":"","state":"OPEN",
        "author":null,"baseRefName":"main","headRefName":"feature","headRefOid":"a".repeat(40),
        "mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","additions":1,"deletions":0,"changedFiles":1
    })).unwrap();
    app.services
        .checkouts
        .entry(key.0.clone())
        .or_default()
        .pull_requests
        .push(details.pull_request.clone());
    app.services.reviews.entry(key.clone()).or_default().details = Some(details);
    (app, key)
}

fn mutation(method: Option<GitHubMergeMethod>) -> Task {
    Task::SetAutoMerge {
        project: "/project".into(),
        url: URL.into(),
        head: "a".repeat(40),
        method,
    }
}

fn result(method: Option<GitHubMergeMethod>) -> Reply {
    Reply::Task(TaskReply::AutoMerge(GitHubAutoMergeResult {
        head_ref_oid: "a".repeat(40),
        state: "OPEN".into(),
        auto_merge_request: method.map(|merge_method| GitHubAutoMergeRequest { merge_method }),
    }))
}

fn start(app: &mut Workspace, task: Task) -> u64 {
    app.task(task);
    let commands = app.commands();
    assert_eq!(commands.len(), 1);
    commands[0].0
}

#[test]
fn auto_merge_confirms_each_setting_and_rejects_details_read_before_or_during_the_change() {
    let (mut app, key) = fixture();
    let refresh = Task::Details {
        project: key.0.clone(),
        url: key.1.clone(),
    };
    for method in [Some(GitHubMergeMethod::Rebase), None] {
        let old_details = app.services.reviews[&key].details.clone().unwrap();
        let before = start(&mut app, refresh.clone());
        let change = mutation(method);
        let request = start(&mut app, change.clone());
        let during = start(&mut app, refresh.clone());
        app.apply(Event::Completed(request, Ok(result(method))));
        let commands = app.commands();
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0].1, Command::Task(Task::Details { project, url }) if (project,url) == (&key.0,&key.1))
        );
        assert!(!app.services.busy(&change));
        assert!(app.services.busy(&refresh));
        for stale in [before, during] {
            app.apply(Event::Completed(
                stale,
                Ok(Reply::Task(TaskReply::Details(old_details.clone()))),
            ));
            assert!(app.services.busy(&refresh));
            let details = app.services.reviews[&key].details.as_ref().unwrap();
            assert_eq!(
                details
                    .auto_merge_request
                    .as_ref()
                    .map(|request| request.merge_method),
                method
            );
        }
        assert_eq!(app.services.checkouts[&key.0].pull_requests.len(), 1);
        let confirmed = app.services.reviews[&key].details.clone().unwrap();
        app.apply(Event::Completed(
            commands[0].0,
            Ok(Reply::Task(TaskReply::Details(confirmed))),
        ));
        app.commands();
    }
    assert_eq!(
        app.services.reviews[&key].notice.as_deref(),
        Some("Auto-merge disabled.")
    );
}

#[test]
fn auto_merge_shares_the_mutation_lock_across_refresh_and_keeps_errors_retryable_and_scoped() {
    let (mut app, key) = fixture();
    let change = mutation(Some(GitHubMergeMethod::Squash));
    let request = start(&mut app, change.clone());
    app.services.refresh_review(&key.0, &key.1);
    for task in [
        change.clone(),
        mutation(None),
        Task::Merge {
            project: key.0.clone(),
            url: key.1.clone(),
            head: "a".repeat(40),
            method: GitHubMergeMethod::Squash,
        },
        Task::UpdateBranch {
            project: key.0.clone(),
            url: key.1.clone(),
            head: "a".repeat(40),
        },
    ] {
        app.task(task);
        assert!(app.commands().is_empty());
    }
    let other = (
        "/other".into(),
        "https://github.com/example/other/pull/44".into(),
    );
    app.services
        .reviews
        .entry(other.clone())
        .or_default()
        .notice = Some("Keep this notice".into());
    app.active_project = Some(other.0.clone());
    app.apply(Event::Completed(
        request,
        Err("Auto-merge is not enabled for this repository".into()),
    ));
    assert!(!app.services.busy(&change));
    assert!(app.services.reviews[&key]
        .error
        .as_ref()
        .unwrap()
        .contains("not enabled"));
    assert!(app.services.reviews[&key]
        .details
        .as_ref()
        .unwrap()
        .auto_merge_request
        .is_none());
    assert_eq!(
        app.services.reviews[&other].notice.as_deref(),
        Some("Keep this notice")
    );
    let retry = start(&mut app, change.clone());
    app.apply(Event::Completed(request, Err("Late duplicate".into())));
    assert!(app.services.busy(&change));
    assert!(app.services.reviews[&key].error.is_none());
    app.apply(Event::Completed(
        retry,
        Ok(result(Some(GitHubMergeMethod::Squash))),
    ));
    assert_eq!(app.active_project.as_deref(), Some("/other"));
    assert!(app.services.reviews[&key]
        .details
        .as_ref()
        .unwrap()
        .auto_merge_request
        .is_some());
    app.commands();
    let disable = start(&mut app, mutation(None));
    assert!(app.services.reviews[&key].notice.is_none());
    app.apply(Event::Completed(disable, Err("Permission denied".into())));
    assert!(app.services.reviews[&key]
        .details
        .as_ref()
        .unwrap()
        .auto_merge_request
        .is_some());
}

#[test]
fn auto_merge_completions_do_not_overwrite_a_newer_review_head() {
    let (mut app, key) = fixture();
    let change = mutation(Some(GitHubMergeMethod::Squash));
    let request = start(&mut app, change.clone());
    app.services.refresh_review(&key.0, &key.1);
    let details = app
        .services
        .reviews
        .get_mut(&key)
        .unwrap()
        .details
        .as_mut()
        .unwrap();
    details.head_ref_oid = "b".repeat(40);
    assert!(app.services.busy(&change));
    app.apply(Event::Completed(
        request,
        Ok(result(Some(GitHubMergeMethod::Squash))),
    ));
    let details = app.services.reviews[&key].details.as_ref().unwrap();
    assert_eq!(details.head_ref_oid, "b".repeat(40));
    assert!(details.auto_merge_request.is_none());
    assert!(app.commands().iter().any(|(_, command)| matches!(command, Command::Task(Task::Details { project, url }) if (project,url) == (&key.0,&key.1))));
}

#[test]
fn auto_merge_admission_rejects_stale_heads_and_closed_or_draft_reviews() {
    for (field, value) in [
        ("headRefOid", json!("b".repeat(40))),
        ("state", json!("CLOSED")),
        ("state", json!("MERGED")),
        ("isDraft", json!(true)),
    ] {
        let (mut app, key) = fixture();
        let mut details =
            serde_json::to_value(app.services.reviews[&key].details.as_ref().unwrap()).unwrap();
        details[field] = value;
        app.services.reviews.get_mut(&key).unwrap().details =
            Some(serde_json::from_value(details).unwrap());
        app.task(mutation(Some(GitHubMergeMethod::Squash)));
        assert!(app.commands().is_empty(), "{field}");
    }
}

#[test]
fn auto_merge_removes_associations_only_when_github_confirms_the_pr_merged() {
    let (mut app, key) = fixture();
    let request = start(&mut app, mutation(Some(GitHubMergeMethod::Squash)));
    app.apply(Event::Completed(
        request,
        Ok(Reply::Task(TaskReply::AutoMerge(GitHubAutoMergeResult {
            head_ref_oid: "a".repeat(40),
            state: "MERGED".into(),
            auto_merge_request: None,
        }))),
    ));
    assert!(app.services.checkouts[&key.0].pull_requests.is_empty());
    assert_eq!(
        app.services.reviews[&key].details.as_ref().unwrap().state,
        "MERGED"
    );
    assert_eq!(
        app.services.reviews[&key].notice.as_deref(),
        Some("Pull request merged.")
    );
}

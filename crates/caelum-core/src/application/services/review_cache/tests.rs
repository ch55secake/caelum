use super::*;

#[test]
fn worker_restores_shared_cache_and_reuses_only_commit_pinned_pages() {
    let directory = std::env::temp_dir().join(format!(
        "caelum-review-worker-{}-{}",
        std::process::id(),
        timestamp()
    ));
    let cache = Cache::for_store(&directory.join("workspace.json"));
    let details: GitHubPullRequestDetails = serde_json::from_value(serde_json::json!({
        "number":42,"title":"Review", "url":"https://git.example.com/team/repo/pull/42",
        "isDraft":false,"body":"Overview","state":"OPEN","baseRefName":"main",
        "baseRefOid":"b".repeat(40),"headRefName":"feature","headRefOid":"a".repeat(40),
        "mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","additions":0,"deletions":0,"changedFiles":0
    })).unwrap();
    let files = GitHubPullRequestFiles {
        files: vec![],
        base_oid: details.base_ref_oid.clone(),
        next_page: None,
    };
    cache.save_details(&details, timestamp());
    cache.save_page(&details.pull_request.url, &details.head_ref_oid, 1, &files);
    let runtime = ProviderRuntime::default();
    let read = Task::ReviewCache {
        project: directory.join("missing-worktree").to_string_lossy().into(),
        url: details.pull_request.url.clone(),
    };
    assert!(
        matches!(read.execute(&runtime), Ok(TaskReply::ReviewCache(None))),
        "a runtime without a store does not touch a default cache"
    );
    *runtime.review_cache.lock().unwrap() =
        Some(Cache::for_store(&directory.join("workspace.json")));
    let Ok(TaskReply::ReviewCache(Some(restored))) = read.execute(&runtime) else {
        panic!("cache miss")
    };
    assert_eq!(restored.details, details);
    assert_eq!(restored.files.as_ref(), Some(&files));
    let mut task = Task::ReviewFiles {
        project: read.project().into(),
        url: details.pull_request.url,
        head: details.head_ref_oid,
        base: details.base_ref_oid,
        page: 1,
    };
    let Ok(TaskReply::ReviewFiles(page)) = task.execute(&runtime) else {
        panic!("cached files must not need gh or a checkout")
    };
    assert_eq!(page, files);
    if let Task::ReviewFiles { base, .. } = &mut task {
        *base = "c".repeat(40);
    }
    assert!(
        task.execute(&runtime).is_err(),
        "another base must go through GitHub"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

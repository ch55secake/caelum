use super::*;
use caelum_protocol::GitHubFeedbackKind;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

const URL: &str = "https://git.example.com/upstream/repo/pull/44";
const HEAD: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";

struct Fixture(PathBuf);
impl Fixture {
    fn new(script: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "caelum-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let fixture = Self(path.canonicalize().unwrap());
        crate::test_support::write_executable(
            &fixture.0.join("gh"),
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >> arguments\n{script}\n"),
        );
        fixture
    }
    fn write(&self, name: &str, value: Value) {
        fs::write(self.0.join(name), value.to_string()).unwrap();
    }
    fn feedback(
        &self,
        kind: GitHubFeedbackKind,
        cursor: Option<&str>,
        thread: Option<&str>,
    ) -> Result<caelum_protocol::GitHubPullRequestFeedback, GitHubError> {
        feedback(&self.0.join("gh"), &self.0, URL, HEAD, kind, cursor, thread)
    }
    fn files(&self, page: u32) -> Result<caelum_protocol::GitHubPullRequestFiles, GitHubError> {
        files(&self.0.join("gh"), &self.0, URL, HEAD, page)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn connection(nodes: Value, cursor: Option<&str>) -> Value {
    json!({"nodes": nodes, "pageInfo": {"hasNextPage": cursor.is_some(), "endCursor": cursor}})
}
fn comment(id: &str) -> Value {
    json!({"id": id, "body": "Please fix **this**.\n```suggestion\nreturn value;\n```", "url": format!("{URL}#discussion_r1"), "author": null, "createdAt": "2026-11-01T12:00:00Z"})
}
fn pr() -> Value {
    json!({"html_url": URL, "head": {"sha": HEAD}, "base": {"sha": "base-commit"}})
}
fn graphql(name: &str, nodes: Value, cursor: Option<&str>) -> Value {
    let mut pr = json!({"url": URL, "headRefOid": HEAD});
    pr[name] = connection(nodes, cursor);
    json!({"data": {"repository": {"pullRequest": pr}}})
}

#[test]
fn files_are_paginated_and_pin_the_canonical_repository_and_head() {
    let fixture =
        Fixture::new("case \"$*\" in *'/files?'*) cat files.json;; *) cat pr.json;; esac");
    fixture.write("pr.json", pr());
    let mut entries = vec![
        json!({"filename":"src/new name.ts", "previous_filename":"src/old name.ts", "status":"renamed", "additions":1, "deletions":1, "patch":"@@ -1 +1 @@\n-old\n+new"});
        30
    ];
    entries[1] = json!({"filename":"image.png", "status":"added", "additions":0, "deletions":0});
    fixture.write("files.json", json!(entries));
    let page = fixture.files(2).unwrap();
    assert_eq!(page.next_page, Some(3));
    assert_eq!(page.base_oid, "base-commit");
    assert_eq!(
        page.files[0].previous_path.as_deref(),
        Some("src/old name.ts")
    );
    assert!(page.files[1].patch.is_none());
    let args = fs::read_to_string(fixture.0.join("arguments")).unwrap();
    assert!(args.contains("--hostname\ngit.example.com\n--method\nGET"));
    assert!(args.contains("repos/upstream/repo/pulls/44/files?per_page=30&page=2"));
    fixture.write("files.json", json!([]));
    assert!(fixture.files(3).unwrap().next_page.is_none());
    assert!(fixture.files(0).is_err());
    assert!(fixture.files(101).is_err());
}

#[test]
fn files_reject_changes_during_loading_and_invalid_targets() {
    let fixture = Fixture::new("case \"$*\" in *'/files?'*) touch read-files; printf '[]';; *) if [ -f read-files ]; then cat changed.json; else cat pr.json; fi;; esac");
    fixture.write("pr.json", pr());
    let mut changed = pr();
    changed["head"]["sha"] = json!("f".repeat(40));
    fixture.write("changed.json", changed);
    assert!(fixture
        .files(1)
        .unwrap_err()
        .to_string()
        .contains("head changed"));
    let mut changed = pr();
    changed["base"]["sha"] = json!("new-base");
    fixture.write("changed.json", changed);
    fs::remove_file(fixture.0.join("read-files")).unwrap();
    assert!(fixture
        .files(1)
        .unwrap_err()
        .to_string()
        .contains("base changed"));
    assert!(matches!(
        files(
            &fixture.0.join("missing"),
            &fixture.0,
            "https://github.com/a/b/issues/44",
            HEAD,
            1
        ),
        Err(GitHubError::InvalidUrl)
    ));
    assert!(matches!(
        files(&fixture.0.join("missing"), &fixture.0, URL, "invalid", 1),
        Err(GitHubError::InvalidHead)
    ));
}

#[test]
fn review_threads_preserve_context_resolution_and_reply_pagination() {
    let fixture = Fixture::new("cat response.json");
    fixture.write(
        "response.json",
        graphql(
            "reviewThreads",
            json!([{
                "id":"thread", "path":"src/a.ts", "line":null, "originalLine":9, "startLine":7,
                "diffSide":"LEFT", "isResolved":true, "isOutdated":true,
                "context": {"nodes":[{"diffHunk":"@@ -7,3 +7,2 @@\n-removed"}]},
                "comments": connection(json!([comment("first")]), Some("reply-cursor"))
            }]),
            Some("thread-cursor"),
        ),
    );
    let result = fixture
        .feedback(
            GitHubFeedbackKind::Threads,
            Some("literal'$(touch unexpected)"),
            None,
        )
        .unwrap();
    assert_eq!(result.next_cursor.as_deref(), Some("thread-cursor"));
    let thread = &result.threads[0];
    assert!(thread.is_resolved && thread.is_outdated);
    assert_eq!(thread.line, None);
    assert_eq!(thread.original_line, Some(9));
    assert_eq!(thread.next_cursor.as_deref(), Some("reply-cursor"));
    assert!(thread.comments[0].body.contains("```suggestion"));
    assert!(thread.comments[0].author.is_none());
    assert!(thread.comments[0]
        .diff_hunk
        .as_deref()
        .unwrap()
        .contains("removed"));
    let args = fs::read_to_string(fixture.0.join("arguments")).unwrap();
    assert!(args.contains("owner=upstream\n-f\nrepo=repo\n-F\nnumber=44"));
    assert!(args.contains("cursor=literal'$(touch unexpected)"));
    assert!(!fixture.0.join("unexpected").exists());
}

#[test]
fn discussion_reviews_and_replies_keep_their_native_identity_and_scope() {
    let fixture = Fixture::new("cat response.json");
    fixture.write(
        "response.json",
        graphql("comments", json!([comment("discussion")]), None),
    );
    assert_eq!(
        fixture
            .feedback(GitHubFeedbackKind::Discussion, None, None)
            .unwrap()
            .comments[0]
            .id,
        "discussion"
    );
    let mut review = comment("review");
    review["state"] = json!("CHANGES_REQUESTED");
    fixture.write("response.json", graphql("reviews", json!([review]), None));
    assert_eq!(
        fixture
            .feedback(GitHubFeedbackKind::Reviews, None, None)
            .unwrap()
            .comments[0]
            .state
            .as_deref(),
        Some("CHANGES_REQUESTED")
    );
    let reply = json!({"data":{"node":{"pullRequest":{"url": URL, "headRefOid":HEAD}, "comments":connection(json!([comment("reply")]),None)}}});
    fixture.write("response.json", reply.clone());
    assert_eq!(
        fixture
            .feedback(GitHubFeedbackKind::Replies, Some("next"), Some("thread"))
            .unwrap()
            .comments[0]
            .id,
        "reply"
    );
    let mut wrong = reply;
    wrong["data"]["node"]["pullRequest"]["url"] = json!("https://github.com/another/repo/pull/44");
    fixture.write("response.json", wrong);
    assert!(fixture
        .feedback(GitHubFeedbackKind::Replies, Some("next"), Some("thread"))
        .unwrap_err()
        .to_string()
        .contains("different pull request"));
}

#[test]
fn partial_graphql_errors_missing_cursors_and_stale_heads_are_not_silent_misses() {
    let fixture = Fixture::new("cat response.json");
    fixture.write(
        "response.json",
        json!({"errors":[{"message":"Rate limited"}]}),
    );
    assert!(fixture
        .feedback(GitHubFeedbackKind::Discussion, None, None)
        .unwrap_err()
        .to_string()
        .contains("Rate limited"));
    let mut response = graphql("comments", json!([]), Some("next"));
    response["data"]["repository"]["pullRequest"]["comments"]["pageInfo"]["endCursor"] =
        Value::Null;
    fixture.write("response.json", response);
    assert!(fixture
        .feedback(GitHubFeedbackKind::Discussion, None, None)
        .unwrap_err()
        .to_string()
        .contains("cursor"));
    let mut response = graphql("comments", json!([]), None);
    response["data"]["repository"]["pullRequest"]["headRefOid"] = json!("f".repeat(40));
    fixture.write("response.json", response);
    assert!(fixture
        .feedback(GitHubFeedbackKind::Discussion, None, None)
        .unwrap_err()
        .to_string()
        .contains("head changed"));
}

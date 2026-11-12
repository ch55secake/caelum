use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

const URL: &str = "https://git.example.com/upstream/repo/pull/44";
const HEAD: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "caelum-auto-merge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let fixture = Self(path.canonicalize().unwrap());
        fixture.script("case \"$*\" in *'query=mutation'*) touch mutated; cat mutation.json;; *) cat lookup.json;; esac");
        fixture.lookup(json!({"id":"PR_example", "url":URL, "headRefOid":HEAD, "state":"OPEN", "isDraft":false}));
        fixture
    }

    fn script(&self, script: &str) {
        crate::test_support::write_executable(&self.0.join("gh"), format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> arguments\npwd > directory\nprintf '%s' \"$GH_PROMPT_DISABLED\" > no-prompts\n{script}\n"
        ));
    }

    fn write(&self, file: &str, value: Value) {
        fs::write(self.0.join(file), value.to_string()).unwrap();
    }

    fn lookup(&self, pr: Value) {
        self.write(
            "lookup.json",
            json!({"data":{"repository":{"pullRequest":pr}}}),
        );
    }

    fn mutation(&self, method: Option<GitHubMergeMethod>, pr: Value) {
        let operation = if method.is_some() {
            "enablePullRequestAutoMerge"
        } else {
            "disablePullRequestAutoMerge"
        };
        self.write(
            "mutation.json",
            json!({"data":{operation:{"pullRequest":pr}}}),
        );
    }

    fn set(&self, method: Option<GitHubMergeMethod>) -> Result<GitHubAutoMergeResult, GitHubError> {
        set_auto_merge(&self.0.join("gh"), &self.0, URL, HEAD, method)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn confirmed(method: Option<&str>) -> Value {
    json!({"url":URL, "headRefOid":HEAD, "state":"OPEN", "autoMergeRequest":method.map(|method| json!({"mergeMethod":method}))})
}

#[test]
fn auto_merge_uses_native_mutations_the_canonical_host_and_the_selected_head_and_method() {
    for (method, name) in [
        (Some(GitHubMergeMethod::Merge), Some("MERGE")),
        (Some(GitHubMergeMethod::Squash), Some("SQUASH")),
        (Some(GitHubMergeMethod::Rebase), Some("REBASE")),
        (None, None),
    ] {
        let fixture = Fixture::new();
        fixture.mutation(method, confirmed(name));
        let result = fixture.set(method).unwrap();
        assert_eq!(result.state, "OPEN");
        assert_eq!(
            result
                .auto_merge_request
                .map(|request| request.merge_method),
            method
        );
        let args = fs::read_to_string(fixture.0.join("arguments")).unwrap();
        assert_eq!(
            args.matches("api\n--hostname\ngit.example.com\ngraphql\n")
                .count(),
            2
        );
        for argument in ["owner=upstream", "repo=repo", "number=44", "id=PR_example"] {
            assert!(args.lines().any(|line| line == argument), "{args}");
        }
        if let Some(name) = name {
            assert!(args.contains("enablePullRequestAutoMerge"));
            assert!(args.lines().any(|line| line == format!("method={name}")));
            assert!(args.lines().any(|line| line == format!("head={HEAD}")));
            assert!(args.contains("expectedHeadOid: $head"));
        } else {
            assert!(args.contains("disablePullRequestAutoMerge"));
            assert!(!args.contains("enablePullRequestAutoMerge"));
        }
        assert!(!args.contains("mergePullRequest("));
        assert_eq!(
            fs::read_to_string(fixture.0.join("no-prompts")).unwrap(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("directory"))
                .unwrap()
                .trim(),
            fixture.0.to_str().unwrap()
        );
    }
}

#[test]
fn auto_merge_rejects_invalid_targets_and_stale_or_closed_reviews_before_mutating() {
    let fixture = Fixture::new();
    for url in [
        "--help",
        "44",
        "http://github.com/a/b/pull/44",
        "https://github.com/a/b/issues/44",
    ] {
        assert!(matches!(
            set_auto_merge(&fixture.0.join("gh"), &fixture.0, url, HEAD, None),
            Err(GitHubError::InvalidUrl)
        ));
    }
    for head in ["", "--help", "not-a-commit"] {
        assert!(matches!(
            set_auto_merge(&fixture.0.join("gh"), &fixture.0, URL, head, None),
            Err(GitHubError::InvalidHead)
        ));
    }
    assert!(!fixture.0.join("arguments").exists());
    for (field, value) in [
        ("url", json!("https://git.example.com/other/repo/pull/44")),
        ("headRefOid", json!("b".repeat(40))),
        ("state", json!("CLOSED")),
        ("state", json!("MERGED")),
        ("isDraft", json!(true)),
        ("id", Value::Null),
    ] {
        let mut pr = json!({"id":"PR_example", "url":URL, "headRefOid":HEAD, "state":"OPEN", "isDraft":false});
        pr[field] = value;
        fixture.lookup(pr);
        assert!(
            fixture.set(Some(GitHubMergeMethod::Squash)).is_err(),
            "{field}"
        );
        assert!(!fixture.0.join("mutated").exists());
    }
}

#[test]
fn auto_merge_preserves_github_errors_and_requires_explicit_confirmation() {
    let method = Some(GitHubMergeMethod::Squash);
    for message in [
        "Authentication required",
        "Auto-merge is not enabled for this repository",
        "Resource not accessible by integration",
        "Head branch was modified",
    ] {
        let fixture = Fixture::new();
        fixture.script(&format!("case \"$*\" in *'query=mutation'*) printf '%s' '{message}' >&2; exit 1;; *) cat lookup.json;; esac"));
        assert!(fixture
            .set(method)
            .unwrap_err()
            .to_string()
            .contains(message));
    }
    let fixture = Fixture::new();
    for response in [
        json!({"data":{"enablePullRequestAutoMerge":{"pullRequest":confirmed(Some("SQUASH"))}}, "errors":[{"message":"Permission denied"}]}),
        json!({"data":{"enablePullRequestAutoMerge":null}}),
        json!({}),
    ] {
        fixture.write("mutation.json", response);
        assert!(fixture.set(method).is_err());
    }
    for pr in [
        confirmed(None),
        confirmed(Some("REBASE")),
        json!({"url":URL, "headRefOid":HEAD, "state":"OPEN"}),
        json!({"url":"https://git.example.com/other/repo/pull/44", "headRefOid":HEAD, "state":"OPEN", "autoMergeRequest":{"mergeMethod":"SQUASH"}}),
    ] {
        fixture.mutation(method, pr);
        assert!(fixture.set(method).is_err());
    }
    fixture.mutation(None, confirmed(Some("SQUASH")));
    assert!(fixture.set(None).is_err());
    fixture.write("lookup.json", json!({"errors":[{"message":"Unavailable"}]}));
    assert!(fixture
        .set(method)
        .unwrap_err()
        .to_string()
        .contains("Unavailable"));
}

#[test]
fn auto_merge_reports_a_pull_request_that_merged_as_requirements_were_met() {
    let fixture = Fixture::new();
    let mut pr = confirmed(None);
    pr["state"] = json!("MERGED");
    fixture.mutation(Some(GitHubMergeMethod::Squash), pr);
    assert_eq!(
        fixture.set(Some(GitHubMergeMethod::Squash)).unwrap().state,
        "MERGED"
    );
}

#[test]
fn details_preserve_enabled_disabled_and_older_auto_merge_metadata() {
    let fixture = Fixture::new();
    fixture.script("cat details.json");
    let mut details = json!({"number":44,"title":"Review","url":URL,"isDraft":false,"body":"","state":"OPEN","author":null,"baseRefName":"main","headRefName":"feature","headRefOid":HEAD,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","additions":1,"deletions":0,"changedFiles":1});
    for method in [None, Some("MERGE"), Some("SQUASH"), Some("REBASE")] {
        details["autoMergeRequest"] = method
            .map(|method| json!({"mergeMethod":method}))
            .unwrap_or(Value::Null);
        fixture.write("details.json", details.clone());
        let result = query_details(
            &fixture.0.join("gh"),
            &fixture.0,
            URL,
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(result.auto_merge_request.is_some(), method.is_some());
        if let Some(method) = method {
            assert_eq!(
                serde_json::to_value(result).unwrap()["autoMergeRequest"]["mergeMethod"],
                method.to_lowercase()
            );
        }
    }
    details.as_object_mut().unwrap().remove("autoMergeRequest");
    fixture.write("details.json", details);
    assert!(query_details(
        &fixture.0.join("gh"),
        &fixture.0,
        URL,
        Duration::from_secs(10)
    )
    .unwrap()
    .auto_merge_request
    .is_none());
    assert!(fs::read_to_string(fixture.0.join("arguments"))
        .unwrap()
        .contains("autoMergeRequest"));
}

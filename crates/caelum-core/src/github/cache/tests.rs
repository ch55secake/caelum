use super::*;
use caelum_protocol::GitHubPullRequestFile;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

const URL: &str = "https://git.example.com/team/repo/pull/42";
const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "caelum-pr-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn cache(&self) -> Cache {
        Cache::for_store(&self.0.join("workspace.json"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn details() -> GitHubPullRequestDetails {
    serde_json::from_value(json!({
        "number":42, "title":"Review cache", "url":URL, "isDraft":false,
        "body":"Overview", "state":"OPEN", "baseRefName":"main", "baseRefOid":BASE,
        "headRefName":"feature", "headRefOid":HEAD, "mergeable":"MERGEABLE",
        "mergeStateStatus":"CLEAN", "additions":1, "deletions":1, "changedFiles":60
    }))
    .unwrap()
}

fn page(index: u32, next: Option<u32>) -> GitHubPullRequestFiles {
    GitHubPullRequestFiles {
        base_oid: BASE.into(),
        next_page: next,
        files: vec![GitHubPullRequestFile {
            path: format!("src/{index}.rs"),
            status: "renamed".into(),
            previous_path: Some(format!("old/{index}.rs")),
            additions: 1,
            deletions: 1,
            patch: Some("@@ -1 +1 @@\n-old\n+new".into()),
        }],
    }
}

#[test]
fn restarted_cache_restores_contiguous_pages_and_never_mixes_revisions_or_repositories() {
    let fixture = Fixture::new();
    fixture.cache().save_details(&details(), timestamp());
    fixture.cache().save_page(URL, HEAD, 1, &page(1, Some(2)));
    fixture.cache().save_page(URL, HEAD, 3, &page(3, None));
    let cached = fixture.cache().load(URL).unwrap();
    assert_eq!(cached.details, details());
    assert_eq!(cached.files.unwrap(), page(1, Some(2)));
    fixture.cache().save_page(URL, HEAD, 2, &page(2, Some(3)));
    let files = fixture.cache().load(URL).unwrap().files.unwrap();
    assert_eq!(files.files.len(), 3);
    assert_eq!(files.next_page, None);
    assert_eq!(files.files[1], page(2, None).files[0]);
    assert!(fixture
        .cache()
        .load("https://git.example.com/fork/repo/pull/42")
        .is_none());
    assert!(fixture
        .cache()
        .load("https://other.example.com/team/repo/pull/42")
        .is_none());
    assert!(fixture
        .cache()
        .page(URL, HEAD, &"c".repeat(40), 1)
        .is_none());
    assert!(fixture
        .cache()
        .page(URL, &"c".repeat(40), BASE, 1)
        .is_none());

    let mut updated = details();
    updated.base_ref_oid = "c".repeat(40);
    fixture.cache().save_details(&updated, timestamp());
    assert!(fixture.cache().load(URL).unwrap().files.is_none());
    assert!(Cache::for_store(&fixture.0.join("isolated.json"))
        .load(URL)
        .is_none());
    assert!(Cache::for_store(&fixture.0.join("workspace.other"))
        .load(URL)
        .is_none());
}

#[test]
fn older_requests_cannot_replace_newer_details_and_expired_entries_miss() {
    let fixture = Fixture::new();
    let now = timestamp();
    let mut newer = details();
    newer.pull_request.title = "New title".into();
    fixture.cache().save_details(&newer, now);
    fixture.cache().save_details(&details(), now - 1);
    assert_eq!(fixture.cache().load(URL).unwrap().details, newer);
    let db = fixture.cache().open(false).unwrap();
    db.execute("UPDATE entries SET observed=?1", [now - MAX_AGE - 1])
        .unwrap();
    assert!(fixture.cache().load(URL).is_none());
    fixture.cache().save_details(&details(), timestamp());
    assert!(fixture.cache().load(URL).is_some());
}

#[test]
fn corruption_unavailable_storage_and_oversized_records_are_cache_misses() {
    let fixture = Fixture::new();
    assert!(fixture.cache().load(URL).is_none());
    assert!(!fixture.0.exists(), "reading a miss creates no files");
    fs::create_dir_all(&fixture.0).unwrap();
    fs::write(&fixture.cache().0, "not a database").unwrap();
    fixture.cache().save_details(&details(), timestamp());
    assert!(fixture.cache().load(URL).is_none());
    fs::remove_file(&fixture.cache().0).unwrap();
    fixture.cache().save_details(&details(), timestamp());
    let db = fixture.cache().open(false).unwrap();
    db.execute("UPDATE entries SET payload=x'7b'", []).unwrap();
    assert!(fixture.cache().load(URL).is_none());
    let mut large = details();
    large.body = "x".repeat(MAX_ENTRY_BYTES);
    fixture.cache().save_details(&large, timestamp());
    assert!(fixture.cache().load(URL).is_none());
    let unavailable = Cache(fixture.0.clone()); // A directory cannot be opened as SQLite.
    unavailable.save_details(&details(), timestamp());
    assert!(unavailable.load(URL).is_none());
}

#[test]
fn eviction_bounds_record_count_bytes_and_reclaims_disk_space() {
    let fixture = Fixture::new();
    let cache = fixture.cache();
    let mut db = cache.open(true).unwrap();
    {
        let tx = db.transaction().unwrap();
        for i in 0..140 {
            tx.execute(
                "INSERT INTO entries VALUES (?1, ?2, ?3, ?4)",
                params![i.to_string(), timestamp(), i, vec![0u8; 8192]],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let before = fs::metadata(&cache.0).unwrap().len();
    let tx = db.transaction().unwrap();
    prune(&tx, 128, MAX_BYTES).unwrap();
    assert_eq!(
        tx.query_row("SELECT count(*) FROM entries", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        128
    );
    // Touch the oldest survivor; byte eviction must now keep it over newer rows.
    tx.execute("UPDATE entries SET used=1000 WHERE key='12'", [])
        .unwrap();
    prune(&tx, 128, 3 * 8192).unwrap();
    assert_eq!(
        tx.query_row(
            "SELECT count(*), sum(length(payload)) FROM entries",
            [],
            |r| Ok((r.get::<_, usize>(0)?, r.get::<_, usize>(1)?))
        )
        .unwrap(),
        (3, 3 * 8192)
    );
    assert_eq!(
        tx.query_row("SELECT count(*) FROM entries WHERE key='12'", [], |r| r
            .get::<_, usize>(
            0
        ))
        .unwrap(),
        1
    );
    tx.commit().unwrap();
    assert!(fs::metadata(&cache.0).unwrap().len() < before / 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&cache.0).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn invalid_identity_and_pagination_cannot_be_reused() {
    let fixture = Fixture::new();
    let cache = fixture.cache();
    cache.save_details(&details(), timestamp());
    cache.save_page(URL, HEAD, 1, &page(1, Some(1)));
    assert!(cache.page(URL, HEAD, BASE, 1).is_none());
    cache.save_page(URL, HEAD, 1, &page(1, None));
    let mut wrong = details();
    wrong.pull_request.url = "https://git.example.com/other/repo/pull/42".into();
    cache
        .put(&key(URL, "", "", 0), &wrong, timestamp())
        .unwrap();
    assert!(cache.load(URL).is_none());
    let mut wrong = page(1, None);
    wrong.base_oid = "c".repeat(40);
    cache
        .put(&key(URL, HEAD, BASE, 1), &wrong, timestamp())
        .unwrap();
    assert!(cache.page(URL, HEAD, BASE, 1).is_none());
}

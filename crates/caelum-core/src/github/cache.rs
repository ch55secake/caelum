//! Best-effort, bounded disk cache. All callers run on service workers.
use super::{pull_request_url, validate_head};
use caelum_protocol::{GitHubPullRequestDetails, GitHubPullRequestFiles};
use rusqlite::{params, Connection, Transaction};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 128;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;
const MAX_AGE: i64 = 7 * 24 * 60 * 60 * 1_000_000_000;

#[derive(Debug, Clone)]
pub struct CachedReview {
    pub details: GitHubPullRequestDetails,
    /// A contiguous prefix, stopping at the first uncached page.
    pub files: Option<GitHubPullRequestFiles>,
}

#[derive(Clone)]
pub(crate) struct Cache(PathBuf);

impl Cache {
    pub(crate) fn for_store(path: &Path) -> Self {
        let mut path = path.as_os_str().to_owned();
        path.push(".pr-cache-v1.sqlite3");
        Self(path.into())
    }

    pub(crate) fn load(&self, url: &str) -> Option<CachedReview> {
        pull_request_url(url).ok()?;
        let mut db = self.open(false)?;
        let tx = db.transaction().ok()?;
        let details: GitHubPullRequestDetails = get(&tx, &key(url, "", "", 0))?;
        if details.pull_request.url != url {
            return None;
        }
        validate_head(&details.head_ref_oid).ok()?;
        validate_head(&details.base_ref_oid).ok()?;
        let mut files: Option<GitHubPullRequestFiles> = None;
        for page in 1..=100 {
            let Some(mut next) =
                read_page(&tx, url, &details.head_ref_oid, &details.base_ref_oid, page)
            else {
                break;
            };
            let more = next.next_page.is_some();
            if let Some(files) = &mut files {
                files.files.append(&mut next.files);
                files.next_page = next.next_page;
            } else {
                files = Some(next);
            }
            if !more {
                break;
            }
        }
        tx.commit().ok()?;
        Some(CachedReview { details, files })
    }

    pub(crate) fn page(
        &self,
        url: &str,
        head: &str,
        base: &str,
        page: u32,
    ) -> Option<GitHubPullRequestFiles> {
        let mut db = self.open(false)?;
        let tx = db.transaction().ok()?;
        let result = read_page(&tx, url, head, base, page)?;
        tx.commit().ok()?;
        Some(result)
    }

    pub(crate) fn save_details(&self, details: &GitHubPullRequestDetails, observed: i64) {
        if pull_request_url(&details.pull_request.url).is_ok()
            && validate_head(&details.head_ref_oid).is_ok()
            && validate_head(&details.base_ref_oid).is_ok()
        {
            let _ = self.put(
                &key(&details.pull_request.url, "", "", 0),
                details,
                observed,
            );
        }
    }

    pub(crate) fn save_page(
        &self,
        url: &str,
        head: &str,
        page: u32,
        files: &GitHubPullRequestFiles,
    ) {
        if pull_request_url(url).is_ok()
            && validate_head(head).is_ok()
            && valid_page(files, &files.base_oid, page)
        {
            let _ = self.put(&key(url, head, &files.base_oid, page), files, timestamp());
        }
    }

    fn open(&self, create: bool) -> Option<Connection> {
        if create {
            if let Some(parent) = self.0.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent).ok()?;
            }
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&self.0).ok()?;
        } else if !self.0.is_file() {
            return None;
        }
        let db = Connection::open(&self.0).ok()?;
        db.busy_timeout(Duration::from_millis(50)).ok()?;
        if create {
            // Reclaim deleted pages on commit; no growing WAL or free-page pool.
            db.execute_batch(
                "PRAGMA auto_vacuum = FULL;
                CREATE TABLE IF NOT EXISTS entries (
                    key TEXT PRIMARY KEY, observed INTEGER NOT NULL,
                    used INTEGER NOT NULL, payload BLOB NOT NULL
                );",
            )
            .ok()?;
        }
        Some(db)
    }

    fn put(&self, key: &str, value: &impl Serialize, observed: i64) -> Option<()> {
        let payload = serde_json::to_vec(value).ok()?;
        if payload.len() > MAX_ENTRY_BYTES {
            return None;
        }
        let mut db = self.open(true)?;
        let tx = db.transaction().ok()?;
        tx.execute(
            "INSERT INTO entries VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(key) DO UPDATE SET observed=excluded.observed,
                used=excluded.used, payload=excluded.payload
            WHERE excluded.observed >= entries.observed",
            params![key, observed, timestamp(), payload],
        )
        .ok()?;
        tx.execute(
            "DELETE FROM entries WHERE observed < ?1",
            [timestamp() - MAX_AGE],
        )
        .ok()?;
        prune(&tx, MAX_ENTRIES, MAX_BYTES)?;
        tx.commit().ok()
    }
}

// Request start time, rather than completion time, prevents a slow older details
// request from replacing a newer snapshot shared by another worktree/process.
pub(crate) fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(i64::MAX as u128) as i64
}

fn key(url: &str, head: &str, base: &str, page: u32) -> String {
    serde_json::json!([url, head, base, page]).to_string()
}

fn get<T: DeserializeOwned>(tx: &Transaction<'_>, key: &str) -> Option<T> {
    let bytes: Vec<u8> = tx
        .query_row(
            "SELECT payload FROM entries WHERE key=?1 AND observed>=?2 AND length(payload)<=?3",
            params![key, timestamp() - MAX_AGE, MAX_ENTRY_BYTES],
            |r| r.get(0),
        )
        .ok()?;
    let value = serde_json::from_slice(&bytes).ok()?;
    tx.execute(
        "UPDATE entries SET used=?2 WHERE key=?1",
        params![key, timestamp()],
    )
    .ok()?;
    Some(value)
}

fn read_page(
    tx: &Transaction<'_>,
    url: &str,
    head: &str,
    base: &str,
    page: u32,
) -> Option<GitHubPullRequestFiles> {
    validate_head(head).ok()?;
    let files = get(tx, &key(url, head, base, page))?;
    valid_page(&files, base, page).then_some(files)
}

fn valid_page(files: &GitHubPullRequestFiles, base: &str, page: u32) -> bool {
    validate_head(base).is_ok()
        && files.base_oid == base
        && (1..=100).contains(&page)
        && files.files.len() <= 30
        && files
            .next_page
            .is_none_or(|next| next == page + 1 && next <= 100)
}

fn prune(tx: &Transaction<'_>, entries: usize, bytes: usize) -> Option<()> {
    let mut query = tx
        .prepare("SELECT key, length(payload) FROM entries ORDER BY used DESC, rowid DESC")
        .ok()?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })
        .ok()?;
    let mut total = 0;
    let mut remove = Vec::new();
    for (index, row) in rows.enumerate() {
        let (key, size) = row.ok()?;
        total += size;
        if index >= entries || total > bytes {
            remove.push(key);
        }
    }
    for key in remove {
        tx.execute("DELETE FROM entries WHERE key=?1", [key]).ok()?;
    }
    Some(())
}

#[cfg(test)]
mod tests;

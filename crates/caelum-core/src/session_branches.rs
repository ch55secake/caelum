use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use caelum_protocol::{CodexEvent, OpenCodeEvent, SessionDetails};

use crate::{database::WorkspaceDatabase, git::GitClient, projects::Project};

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    path: String,
    provider: String,
    session: String,
}

struct Execution {
    root: PathBuf,
    turn: Option<String>,
    ambiguous: bool,
}

#[derive(Default)]
struct Tracking {
    branches: HashMap<Key, Option<String>>,
    active: HashMap<Key, Execution>,
}

/// Attribute checkout branches to actual execution, never to sidebar reads.
#[derive(Clone)]
pub struct SessionBranches {
    database: Option<WorkspaceDatabase>,
    tracking: Arc<Mutex<Tracking>>,
}

impl SessionBranches {
    pub fn new(database: Option<WorkspaceDatabase>) -> Self {
        Self {
            database,
            tracking: Arc::new(Mutex::new(Tracking::default())),
        }
    }

    pub fn codex_event(&self, event: &CodexEvent) {
        if event.method == "codex/processExited" {
            self.disconnect(&event.project_path, "codex");
        }
        let Some(id) = event.params.get("threadId").and_then(|id| id.as_str()) else {
            return;
        };
        let turn = event.params.pointer("/turn/id").and_then(|id| id.as_str());
        match event.method.as_str() {
            "turn/started" => self.execution(&event.project_path, "codex", id, turn, true),
            "turn/completed" => self.execution(&event.project_path, "codex", id, turn, false),
            _ => {}
        }
    }

    pub fn opencode_event(&self, event: &OpenCodeEvent) {
        if event.event_type == "server.disconnected" {
            self.disconnect(&event.project_path, "opencode");
        }
        let Some(id) = event.data.get("sessionID").and_then(|id| id.as_str()) else {
            return;
        };
        match event.event_type.as_str() {
            "session.execution.started" => {
                self.execution(&event.project_path, "opencode", id, None, true);
            }
            "session.execution.succeeded"
            | "session.execution.failed"
            | "session.execution.interrupted" => {
                self.execution(&event.project_path, "opencode", id, None, false);
            }
            _ => {}
        }
    }

    pub fn details(
        &self,
        path: &str,
        provider: &str,
        id: &str,
        mut details: SessionDetails,
    ) -> SessionDetails {
        let Some(key) = key(path, provider, id) else {
            return details;
        };
        let tracking = self
            .tracking
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = tracking.branches.get(&key).cloned().or_else(|| {
            self.database
                .as_ref()?
                .session_branch(&key.path, provider, id)
                .ok()
                .flatten()
        });
        if let Some(branch) = saved {
            details.branch = branch;
        } else if details.branch_is_current {
            // A directory's current branch says nothing about an older session.
            details.branch = None;
        }
        details.branch_is_current = false;
        details
    }

    fn execution(&self, path: &str, provider: &str, id: &str, turn: Option<&str>, starting: bool) {
        let Some(key) = key(path, provider, id) else {
            return;
        };
        let mut tracking = self
            .tracking
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !starting {
            let Some(active) = tracking.active.get(&key) else {
                return;
            };
            if active.turn.as_deref() != turn {
                return;
            }
            let active = tracking.active.remove(&key).unwrap();
            if active.ambiguous {
                return;
            }
            if let Ok(Some(context)) = GitClient::new().branch_context(&key.path) {
                if context.root == active.root {
                    self.record(&mut tracking, key, context.branch);
                }
            }
            return;
        }
        if tracking
            .active
            .get(&key)
            .is_some_and(|active| active.turn.as_deref() == turn)
        {
            return;
        }
        let Ok(Some(context)) = GitClient::new().branch_context(&key.path) else {
            return;
        };
        let mut ambiguous = false;
        for (other, active) in &mut tracking.active {
            if other != &key && active.root == context.root {
                active.ambiguous = true;
                ambiguous = true;
            }
        }
        tracking.active.insert(
            key.clone(),
            Execution {
                root: context.root,
                turn: turn.map(str::to_owned),
                ambiguous,
            },
        );
        // Overlapping sessions in one checkout cannot reliably own its branch changes.
        if !ambiguous {
            self.record(&mut tracking, key, context.branch);
        }
    }

    fn record(&self, tracking: &mut Tracking, key: Key, branch: Option<String>) {
        if let Some(database) = &self.database {
            let _ = database.record_session_branch(
                &key.path,
                &key.provider,
                &key.session,
                branch.as_deref(),
            );
        }
        tracking.branches.insert(key, branch);
    }

    fn disconnect(&self, path: &str, provider: &str) {
        let mut tracking = self
            .tracking
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        tracking
            .active
            .retain(|key, _| key.path != path || key.provider != provider);
    }
}

fn key(path: &str, provider: &str, id: &str) -> Option<Key> {
    if id.is_empty() {
        return None;
    }
    Some(Key {
        path: Project::open(path)
            .ok()?
            .path()
            .to_string_lossy()
            .into_owned(),
        provider: provider.to_owned(),
        session: id.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use caelum_protocol::LineChanges;
    use serde_json::json;
    use std::{
        fs,
        path::Path,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "caelum-session-branches-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            let fixture = Self(path.canonicalize().unwrap());
            git(&fixture.0, &["init", "-b", "main"]);
            git(
                &fixture.0,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "Initial",
                ],
            );
            fixture
        }
        fn database(&self) -> WorkspaceDatabase {
            WorkspaceDatabase::open(self.0.join("workspace.sqlite3")).unwrap()
        }
        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn git(path: &Path, args: &[&str]) {
        let result = Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fn native(branch: Option<&str>, current: bool) -> SessionDetails {
        SessionDetails {
            branch: branch.map(str::to_owned),
            branch_is_current: current,
            changes: Some(LineChanges {
                additions: 3,
                deletions: 1,
            }),
        }
    }
    fn codex(tracker: &SessionBranches, path: &str, session: &str, turn: &str, method: &str) {
        tracker.codex_event(&CodexEvent {
            project_path: path.into(),
            method: method.into(),
            params: json!({"threadId": session, "turn": {"id": turn}}),
            request_id: None,
        });
    }
    fn opencode(tracker: &SessionBranches, path: &str, session: &str, event: &str) {
        tracker.opencode_event(&OpenCodeEvent {
            project_path: path.into(),
            event_type: event.into(),
            data: json!({"sessionID": session}),
            id: None,
            created: None,
            location: None,
        });
    }

    #[test]
    fn execution_records_new_branches_and_survives_navigation_archive_and_restart() {
        let fixture = Fixture::new();
        let database = fixture.database();
        let tracker = SessionBranches::new(Some(database.clone()));
        let path = fixture.path();
        assert_eq!(
            tracker
                .details(path, "codex", "same-id", native(Some("recorded"), false))
                .branch
                .as_deref(),
            Some("recorded")
        );
        assert_eq!(
            tracker
                .details(path, "opencode", "same-id", native(Some("main"), true))
                .branch,
            None
        );
        codex(&tracker, path, "same-id", "turn-1", "turn/started");
        git(
            &fixture.0,
            &["checkout", "-b", "feature/created-during-turn"],
        );
        codex(&tracker, path, "same-id", "turn-1", "turn/completed");
        git(&fixture.0, &["checkout", "-b", "feature/other-session"]);
        opencode(&tracker, path, "same-id", "session.execution.started");
        opencode(&tracker, path, "same-id", "session.execution.succeeded");
        let details = tracker.details(path, "codex", "same-id", native(Some("main"), false));
        assert_eq!(
            details.branch.as_deref(),
            Some("feature/created-during-turn")
        );
        assert!(!details.branch_is_current);
        assert_eq!(details.changes, native(None, false).changes);
        database
            .remember_project(&Project::open(path).unwrap().info())
            .unwrap();
        database
            .select_session(path, "opencode", Some("same-id"))
            .unwrap();
        database
            .remove_session(path, "opencode", "same-id")
            .unwrap();
        database.sync_sessions(path, "opencode", &[]).unwrap();
        drop(tracker);
        drop(database);
        git(&fixture.0, &["checkout", "main"]);
        let restored = SessionBranches::new(Some(fixture.database()));
        assert_eq!(
            restored
                .details(path, "codex", "same-id", native(Some("main"), false))
                .branch
                .as_deref(),
            Some("feature/created-during-turn")
        );
        assert_eq!(
            restored
                .details(path, "opencode", "same-id", native(Some("main"), true))
                .branch
                .as_deref(),
            Some("feature/other-session")
        );
        assert_eq!(
            restored
                .details(path, "opencode", "unobserved", native(Some("main"), true))
                .branch,
            None
        );
    }

    #[test]
    fn overlapping_sessions_in_the_same_checkout_do_not_claim_each_others_branch() {
        let fixture = Fixture::new();
        let tracker = SessionBranches::new(Some(fixture.database()));
        let nested = fixture.0.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let nested = nested.to_str().unwrap();
        codex(&tracker, fixture.path(), "first", "turn-1", "turn/started");
        git(&fixture.0, &["checkout", "-b", "feature/ambiguous"]);
        opencode(&tracker, nested, "second", "session.execution.started");
        codex(
            &tracker,
            fixture.path(),
            "first",
            "turn-1",
            "turn/completed",
        );
        opencode(&tracker, nested, "second", "session.execution.succeeded");
        assert_eq!(
            tracker
                .details(fixture.path(), "codex", "first", native(None, false))
                .branch
                .as_deref(),
            Some("main")
        );
        assert_eq!(
            tracker
                .details(nested, "opencode", "second", native(None, false))
                .branch,
            None
        );
        opencode(&tracker, nested, "second", "session.execution.started");
        opencode(&tracker, nested, "second", "session.execution.failed");
        assert_eq!(
            tracker
                .details(nested, "opencode", "second", native(None, false))
                .branch
                .as_deref(),
            Some("feature/ambiguous")
        );
    }

    #[test]
    fn linked_worktrees_are_independent_even_with_the_same_provider_session_id() {
        let fixture = Fixture::new();
        let checkout = fixture.0.join("checkout");
        git(
            &fixture.0,
            &[
                "worktree",
                "add",
                "-b",
                "feature/worktree",
                checkout.to_str().unwrap(),
            ],
        );
        let tracker = SessionBranches::new(Some(fixture.database()));
        let path = checkout.to_str().unwrap();
        codex(&tracker, fixture.path(), "same-id", "a", "turn/started");
        codex(&tracker, path, "same-id", "b", "turn/started");
        git(&fixture.0, &["checkout", "-b", "feature/root"]);
        git(&checkout, &["checkout", "-b", "feature/checkout"]);
        codex(&tracker, fixture.path(), "same-id", "a", "turn/completed");
        codex(&tracker, path, "same-id", "b", "turn/completed");
        assert_eq!(
            tracker
                .details(fixture.path(), "codex", "same-id", native(None, false))
                .branch
                .as_deref(),
            Some("feature/root")
        );
        assert_eq!(
            tracker
                .details(path, "codex", "same-id", native(None, false))
                .branch
                .as_deref(),
            Some("feature/checkout")
        );
    }

    #[test]
    fn stale_completions_errors_and_detached_heads_do_not_restore_a_previous_branch() {
        let fixture = Fixture::new();
        let tracker = SessionBranches::new(Some(fixture.database()));
        let path = fixture.path();
        codex(&tracker, path, "session", "old", "turn/started");
        codex(&tracker, path, "session", "new", "turn/started");
        git(&fixture.0, &["checkout", "-b", "feature/new"]);
        codex(&tracker, path, "session", "old", "turn/completed");
        assert_eq!(
            tracker
                .details(path, "codex", "session", native(None, false))
                .branch
                .as_deref(),
            Some("main")
        );
        codex(&tracker, path, "session", "new", "turn/completed");
        codex(&tracker, path, "session", "error", "turn/started");
        fs::rename(fixture.0.join(".git"), fixture.0.join("hidden-git")).unwrap();
        codex(&tracker, path, "session", "error", "turn/completed");
        assert_eq!(
            tracker
                .details(path, "codex", "session", native(None, false))
                .branch
                .as_deref(),
            Some("feature/new")
        );
        fs::rename(fixture.0.join("hidden-git"), fixture.0.join(".git")).unwrap();
        codex(&tracker, path, "session", "detach", "turn/started");
        git(&fixture.0, &["checkout", "--detach"]);
        codex(&tracker, path, "session", "detach", "turn/completed");
        let restored = SessionBranches::new(Some(fixture.database()));
        assert_eq!(
            restored
                .details(path, "codex", "session", native(Some("main"), false))
                .branch,
            None
        );
    }
}

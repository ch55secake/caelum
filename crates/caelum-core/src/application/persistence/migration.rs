use super::*;
use crate::database::WorkspaceDatabase;
use rusqlite::{Connection, DatabaseName, OpenFlags};

impl Store {
    /// Only the production default imports old data. Explicit state paths stay isolated.
    pub(crate) fn migrate_default(&self) -> Result<(), String> {
        if std::env::var_os("CAELUM_STATE").is_some()
            || std::env::var_os("CAELUM_RUNE_STATE").is_some()
            || self.0 != Self::default_path()
        {
            return Ok(());
        }
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        self.migrate_from(
            &home.join(".caelum/rune/workspace-v1.json"),
            &home.join("Library/Application Support/dev.caelum.desktop/workspace.sqlite3"),
        )
    }

    fn migrate_from(&self, candidate: &Path, legacy: &Path) -> Result<(), String> {
        if self.0.try_exists().map_err(|e| e.to_string())? {
            return Ok(());
        }
        let snapshot = if let Some(snapshot) = Store(candidate.into()).load()? {
            // Candidate drafts, attachments, pending sends and queues always take precedence.
            self.import_database(&candidate.with_file_name("branches.sqlite3"))?;
            Some(snapshot)
        } else if legacy.try_exists().map_err(|e| e.to_string())? {
            let database = self.import_database(legacy)?.expect("existing database");
            Some(from_legacy(database.restore().map_err(|e| e.to_string())?))
        } else {
            None
        };
        if let Some(snapshot) = snapshot {
            self.save(&snapshot)?;
        }
        Ok(())
    }

    fn import_database(&self, source: &Path) -> Result<Option<WorkspaceDatabase>, String> {
        if !source.try_exists().map_err(|e| e.to_string())? {
            return Ok(None);
        }
        let target = self.0.with_file_name("branches.sqlite3");
        let parent = target.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        if !target.try_exists().map_err(|e| e.to_string())? {
            let temporary = parent.join(format!("migration-{}.sqlite3", std::process::id()));
            let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| e.to_string())?;
            // SQLite's backup includes committed WAL data; the original is never migrated or moved.
            let result = source
                .backup(DatabaseName::Main, &temporary, None)
                .map_err(|e| e.to_string())
                .and_then(|()| fs::rename(&temporary, &target).map_err(|e| e.to_string()));
            if result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            result?;
        }
        WorkspaceDatabase::open(target)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

fn from_legacy(old: caelum_protocol::WorkspaceSnapshot) -> Snapshot {
    let mut state = crate::application::Workspace::default().snapshot();
    let active = old
        .projects
        .iter()
        .find(|r| Some(&r.project.path) == old.active_project_path.as_ref())
        .and_then(|r| r.selected_worktree_path.clone())
        .or(old.active_project_path);
    for record in old.projects {
        let path = record.project.path.clone();
        if let Some(root) = record.root_path {
            state.project_roots.insert(path.clone(), root);
        }
        state.projects.push(record.project);
        let preferred = if record.provider == "opencode" {
            Provider::OpenCode
        } else {
            Provider::Codex
        };
        let mut selected = None;
        for (provider, ids, chosen) in [
            (
                Provider::Codex,
                record.codex_session_ids,
                record.codex_session_id,
            ),
            (
                Provider::OpenCode,
                record.opencode_session_ids,
                record.opencode_session_id,
            ),
        ] {
            let mut ids = ids;
            if let Some(id) = chosen.as_ref().filter(|id| !ids.contains(id)) {
                ids.insert(0, id.clone());
            }
            for id in ids {
                let index = state.sessions.len();
                if provider == preferred && (Some(&id) == chosen.as_ref() || selected.is_none()) {
                    selected = Some(index);
                }
                state.sessions.push(saved(
                    Scope {
                        project: path.clone(),
                        provider,
                    },
                    Some(id),
                ));
            }
        }
        let selected = selected.unwrap_or_else(|| {
            let index = state.sessions.len();
            state.sessions.push(saved(
                Scope {
                    project: path.clone(),
                    provider: preferred,
                },
                None,
            ));
            index
        });
        state.remembered.insert(path.clone(), selected);
        if active.as_ref() == Some(&path) {
            state.selected = Some(selected);
        }
    }
    state.selected = state
        .selected
        .or_else(|| (!state.sessions.is_empty()).then_some(0));
    state
}

fn saved(scope: Scope, remote_id: Option<String>) -> SavedSession {
    SavedSession {
        scope,
        remote_id,
        title: "Restored session".into(),
        options: Options::default(),
        draft: String::new(),
        attachments: vec![],
        failed_prompt: None,
        pending_submission: None,
        queue: vec![],
        inbox: vec![],
        archived: false,
        reading: -1.0,
        unread: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::Workspace;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            // Wall-clock timestamps can coincide across parallel test threads.
            let path = std::env::temp_dir().join(format!(
                "caelum-migration-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create isolated migration fixture");
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn candidate_migration_preserves_every_draft_and_never_reimports() {
        let root = Temp::new();
        let source = Store(root.0.join("candidate/workspace-v1.json"));
        let destination = Store(root.0.join("production/workspace-v1.json"));
        let mut app = Workspace::default();
        let i = app.new_session(Scope {
            project: "/project".into(),
            provider: Provider::Codex,
        });
        app.sessions[i].draft = "newer café draft".into();
        app.sessions[i].attachments.push(Attachment::new(
            "src/main.rs".into(),
            "captured context".into(),
            None,
            false,
        ));
        app.sessions[i].failed_prompt = Some("failed send".into());
        app.sessions[i].queue.push(QueuedPrompt {
            text: "queued".into(),
            options: Options::default(),
        });
        let mut expected = app.snapshot();
        expected.sessions[i].pending_submission = Some(QueuedPrompt {
            text: "unacknowledged send".into(),
            options: Options::default(),
        });
        source.save(&expected).unwrap();
        let legacy = root.0.join("legacy.sqlite3");
        let database = WorkspaceDatabase::open(&legacy).unwrap();
        database
            .remember_project(&ProjectInfo {
                name: "Older workspace".into(),
                path: "/older".into(),
            })
            .unwrap();
        let branches =
            WorkspaceDatabase::open(source.0.with_file_name("branches.sqlite3")).unwrap();
        branches
            .record_session_branch("/project", "codex", "thread", Some("candidate-branch"))
            .unwrap();
        destination.migrate_from(&source.0, &legacy).unwrap();
        assert_eq!(destination.load().unwrap(), Some(expected.clone()));
        assert_eq!(source.load().unwrap(), Some(expected.clone()));
        let copied =
            WorkspaceDatabase::open(destination.0.with_file_name("branches.sqlite3")).unwrap();
        assert_eq!(
            copied
                .session_branch("/project", "codex", "thread")
                .unwrap(),
            branches
                .session_branch("/project", "codex", "thread")
                .unwrap()
        );
        source.save(&Workspace::default().snapshot()).unwrap();
        destination
            .migrate_from(&source.0, &root.0.join("missing.sqlite3"))
            .unwrap();
        assert_eq!(destination.load().unwrap(), Some(expected));
        let lock = destination.lock().unwrap();
        assert!(destination.lock().is_err());
        // A duplicate keeps the underlying file description alive, just like a
        // child forked by a parallel provider test before its close-on-exec.
        let inherited = lock.0.try_clone().unwrap();
        drop(lock);
        let replacement = destination.lock().unwrap();
        assert!(destination.lock().is_err());
        drop(inherited);
        assert!(destination.lock().is_err());
        drop(replacement);
        assert!(destination.lock().is_ok());
    }

    #[test]
    fn legacy_workspace_and_branch_history_are_copied_without_changing_source() {
        let root = Temp::new();
        let legacy = root.0.join("legacy/workspace.sqlite3");
        let database = WorkspaceDatabase::open(&legacy).unwrap();
        let wal = Connection::open(&legacy).unwrap();
        wal.pragma_update(None, "journal_mode", "WAL").unwrap();
        let project = ProjectInfo {
            name: "Project".into(),
            path: "/project".into(),
        };
        let checkout = ProjectInfo {
            name: "Worktree".into(),
            path: "/worktree".into(),
        };
        database.remember_project(&project).unwrap();
        database
            .select_session("/project", "codex", Some("thread"))
            .unwrap();
        database.remember_worktree(&project, &checkout).unwrap();
        database
            .select_session("/worktree", "opencode", Some("session"))
            .unwrap();
        database.set_provider("/worktree", "opencode").unwrap();
        database.set_active_project(Some("/worktree")).unwrap();
        database
            .record_session_branch("/project", "codex", "thread", Some("feature"))
            .unwrap();
        database
            .record_session_branch("/project", "codex", "thread", Some("next"))
            .unwrap();
        let before = fs::read(&legacy).unwrap();
        let wal_path = legacy.with_file_name("workspace.sqlite3-wal");
        let before_wal = fs::read(&wal_path).unwrap();
        assert!(!before_wal.is_empty());
        let destination = Store(root.0.join("production/workspace-v1.json"));
        destination
            .migrate_from(&root.0.join("absent.json"), &legacy)
            .unwrap();
        let state = destination.load().unwrap().unwrap();
        assert!(state.projects.contains(&project));
        assert!(state.projects.contains(&checkout));
        assert_eq!(state.project_roots["/worktree"], "/project");
        assert_eq!(
            state.sessions[state.selected.unwrap()].remote_id.as_deref(),
            Some("session")
        );
        assert_eq!(
            state.sessions[state.remembered["/project"]]
                .remote_id
                .as_deref(),
            Some("thread")
        );
        assert_eq!(fs::read(&legacy).unwrap(), before);
        assert_eq!(fs::read(&wal_path).unwrap(), before_wal);
        assert!(destination.0.with_file_name("branches.sqlite3").is_file());
        let imported =
            WorkspaceDatabase::open(destination.0.with_file_name("branches.sqlite3")).unwrap();
        assert_eq!(
            imported
                .session_branch("/project", "codex", "thread")
                .unwrap()
                .unwrap()
                .branches,
            vec!["feature", "next"]
        );
    }

    #[test]
    fn corrupt_or_future_candidate_data_never_becomes_an_empty_workspace() {
        let root = Temp::new();
        fs::create_dir_all(&root.0).unwrap();
        let source = root.0.join("candidate.json");
        let destination = Store(root.0.join("production/workspace-v1.json"));
        fs::write(&source, "broken").unwrap();
        assert!(destination
            .migrate_from(&source, &root.0.join("absent"))
            .is_err());
        assert!(!destination.0.exists());
        assert_eq!(fs::read_to_string(&source).unwrap(), "broken");
        let mut future = Workspace::default().snapshot();
        future.version = 99;
        Store(source.clone()).save(&future).unwrap();
        assert!(destination
            .migrate_from(&source, &root.0.join("absent"))
            .is_err());
        assert!(!destination.0.exists());
        let bytes = fs::read(&source).unwrap();
        assert_eq!(serde_json::from_slice::<Snapshot>(&bytes).unwrap(), future);
        // An existing destination also remains authoritative, even if unreadable.
        fs::create_dir_all(destination.0.parent().unwrap()).unwrap();
        fs::write(&destination.0, "damaged production state").unwrap();
        destination
            .migrate_from(&source, &root.0.join("absent"))
            .unwrap();
        assert!(destination.load().is_err());
        assert_eq!(
            fs::read_to_string(&destination.0).unwrap(),
            "damaged production state"
        );
    }
}

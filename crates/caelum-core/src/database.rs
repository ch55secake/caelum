use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use caelum_protocol::{PersistedProject, ProjectInfo, WorkspaceSnapshot};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use thiserror::Error;

const CURRENT_SCHEMA_VERSION: i64 = 3;
const PROVIDERS: [&str; 2] = ["codex", "opencode"];

/// Thread-safe access to Caelum's app-owned workspace metadata database.
/// Provider conversation history is deliberately never stored here.
#[derive(Clone)]
pub struct WorkspaceDatabase {
    connection: Arc<Mutex<Connection>>,
}

impl WorkspaceDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| DatabaseError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut connection)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn restore(&self) -> Result<WorkspaceSnapshot, DatabaseError> {
        let connection = self.lock();
        let mut statement = connection.prepare(
            "SELECT path, name, provider
             FROM projects
             WHERE is_open = 1
             ORDER BY open_order ASC, last_opened_at DESC, path ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut projects = Vec::new();
        for row in rows {
            let (path, name, provider) = row?;
            validate_provider(&provider)?;
            let mut provider_statement = connection.prepare(
                "SELECT provider, selected_session_id
                 FROM project_provider_state
                 WHERE project_path = ?1",
            )?;
            let provider_rows = provider_statement.query_map([&path], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?;

            let (mut codex_session_id, mut opencode_session_id) = (None, None);
            for provider_row in provider_rows {
                let (provider, session_id) = provider_row?;
                match provider.as_str() {
                    "codex" => codex_session_id = session_id,
                    "opencode" => opencode_session_id = session_id,
                    _ => return Err(DatabaseError::UnsupportedProvider(provider)),
                }
            }

            let mut session_statement = connection.prepare(
                "SELECT provider, session_id
                 FROM sessions
                 WHERE project_path = ?1
                 ORDER BY COALESCE(last_opened_at, 0) DESC, first_seen_at DESC, session_id ASC",
            )?;
            let session_rows = session_statement.query_map([&path], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let (mut codex_session_ids, mut opencode_session_ids) = (Vec::new(), Vec::new());
            for session_row in session_rows {
                let (provider, session_id) = session_row?;
                match provider.as_str() {
                    "codex" => codex_session_ids.push(session_id),
                    "opencode" => opencode_session_ids.push(session_id),
                    _ => return Err(DatabaseError::UnsupportedProvider(provider)),
                }
            }

            projects.push(PersistedProject {
                project: ProjectInfo { name, path },
                provider,
                codex_session_id,
                opencode_session_id,
                codex_session_ids,
                opencode_session_ids,
            });
        }

        let active_project_path = connection
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'active_project_path'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .filter(|path| projects.iter().any(|project| project.project.path == *path));

        Ok(WorkspaceSnapshot {
            projects,
            active_project_path,
        })
    }

    /// Record a project as open and move it to the front of the restore order.
    pub fn remember_project(&self, project: &ProjectInfo) -> Result<(), DatabaseError> {
        validate_nonempty("project path", &project.path)?;
        validate_nonempty("project name", &project.name)?;
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let now = unix_timestamp();
        transaction.execute(
            "UPDATE projects SET open_order = open_order + 1 WHERE is_open = 1",
            [],
        )?;
        transaction.execute(
            "INSERT INTO projects (path, name, is_open, open_order, last_opened_at)
             VALUES (?1, ?2, 1, 0, ?3)
             ON CONFLICT(path) DO UPDATE SET
                 name = excluded.name,
                 is_open = 1,
                 open_order = 0,
                 last_opened_at = excluded.last_opened_at",
            params![project.path, project.name, now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Close a project in the restored workspace without discarding its session metadata.
    pub fn close_project(
        &self,
        path: &str,
        next_active_project_path: Option<&str>,
    ) -> Result<(), DatabaseError> {
        validate_nonempty("project path", path)?;
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        transaction.execute("UPDATE projects SET is_open = 0 WHERE path = ?1", [path])?;
        set_active_project_in_transaction(&transaction, next_active_project_path)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn set_active_project(&self, path: Option<&str>) -> Result<(), DatabaseError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        set_active_project_in_transaction(&transaction, path)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn set_provider(&self, path: &str, provider: &str) -> Result<(), DatabaseError> {
        validate_nonempty("project path", path)?;
        validate_provider(provider)?;
        let connection = self.lock();
        let changed = connection.execute(
            "UPDATE projects SET provider = ?2 WHERE path = ?1",
            params![path, provider],
        )?;
        if changed == 0 {
            return Err(DatabaseError::UnknownProject(path.to_owned()));
        }
        Ok(())
    }

    pub fn select_session(
        &self,
        path: &str,
        provider: &str,
        session_id: Option<&str>,
    ) -> Result<(), DatabaseError> {
        validate_nonempty("project path", path)?;
        validate_provider(provider)?;
        if let Some(session_id) = session_id {
            validate_nonempty("session ID", session_id)?;
            if session_id.len() > 1024 {
                return Err(DatabaseError::InvalidValue("session ID is too long"));
            }
        }

        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE path = ?1)",
            [path],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(DatabaseError::UnknownProject(path.to_owned()));
        }

        transaction.execute(
            "INSERT INTO project_provider_state (project_path, provider, selected_session_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_path, provider) DO UPDATE SET
                 selected_session_id = excluded.selected_session_id",
            params![path, provider, session_id],
        )?;

        if let Some(session_id) = session_id {
            transaction.execute(
                "INSERT INTO sessions (project_path, provider, session_id, first_seen_at, last_opened_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(project_path, provider, session_id) DO UPDATE SET
                     last_opened_at = excluded.last_opened_at",
                params![path, provider, session_id, unix_timestamp()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Forget one archived session and clear its selection atomically.
    pub fn remove_session(
        &self,
        path: &str,
        provider: &str,
        session_id: &str,
    ) -> Result<(), DatabaseError> {
        validate_nonempty("project path", path)?;
        validate_provider(provider)?;
        validate_nonempty("session ID", session_id)?;
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM sessions WHERE project_path = ?1 AND provider = ?2 AND session_id = ?3",
            params![path, provider, session_id],
        )?;
        transaction.execute(
            "UPDATE project_provider_state SET selected_session_id = NULL WHERE project_path = ?1 AND provider = ?2 AND selected_session_id = ?3",
            params![path, provider, session_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Record provider-owned session identifiers without copying provider history or content.
    pub fn sync_sessions(
        &self,
        path: &str,
        provider: &str,
        session_ids: &[String],
    ) -> Result<(), DatabaseError> {
        validate_nonempty("project path", path)?;
        validate_provider(provider)?;
        for session_id in session_ids {
            validate_nonempty("session ID", session_id)?;
            if session_id.len() > 1024 {
                return Err(DatabaseError::InvalidValue("session ID is too long"));
            }
        }

        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE path = ?1)",
            [path],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(DatabaseError::UnknownProject(path.to_owned()));
        }

        let retained = session_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let stale_sessions = {
            let mut statement = transaction.prepare(
                "SELECT session_id FROM sessions WHERE project_path = ?1 AND provider = ?2",
            )?;
            let rows =
                statement.query_map(params![path, provider], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for session_id in stale_sessions {
            if !retained.contains(session_id.as_str()) {
                transaction.execute(
                    "DELETE FROM sessions
                     WHERE project_path = ?1 AND provider = ?2 AND session_id = ?3",
                    params![path, provider, session_id],
                )?;
            }
        }

        let now = unix_timestamp();
        for session_id in session_ids {
            transaction.execute(
                "INSERT INTO sessions (project_path, provider, session_id, first_seen_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(project_path, provider, session_id) DO NOTHING",
                params![path, provider, session_id, now],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn set_active_project_in_transaction(
    transaction: &Transaction<'_>,
    path: Option<&str>,
) -> Result<(), DatabaseError> {
    if let Some(path) = path {
        validate_nonempty("project path", path)?;
        let is_open = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE path = ?1 AND is_open = 1)",
            [path],
            |row| row.get::<_, bool>(0),
        )?;
        if !is_open {
            return Err(DatabaseError::UnknownProject(path.to_owned()));
        }
        transaction.execute(
            "INSERT INTO app_metadata (key, value) VALUES ('active_project_path', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [path],
        )?;
    } else {
        transaction.execute(
            "DELETE FROM app_metadata WHERE key = 'active_project_path'",
            [],
        )?;
    }
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), DatabaseError> {
    let mut version =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::UnsupportedSchema(version));
    }

    if version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE projects (
                path TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                is_open INTEGER NOT NULL DEFAULT 1 CHECK (is_open IN (0, 1)),
                open_order INTEGER NOT NULL DEFAULT 0,
                last_opened_at INTEGER NOT NULL,
                provider TEXT NOT NULL DEFAULT 'codex' CHECK (provider IN ('codex', 'opencode'))
             );",
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
        version = 1;
    }

    if version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE project_provider_state (
                project_path TEXT NOT NULL REFERENCES projects(path) ON DELETE CASCADE,
                provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                selected_session_id TEXT,
                PRIMARY KEY (project_path, provider)
             );
             CREATE TABLE sessions (
                project_path TEXT NOT NULL REFERENCES projects(path) ON DELETE CASCADE,
                provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                session_id TEXT NOT NULL,
                first_seen_at INTEGER NOT NULL,
                last_opened_at INTEGER,
                local_label TEXT,
                PRIMARY KEY (project_path, provider, session_id)
             );
             CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
             );
             CREATE INDEX sessions_recent_idx
                ON sessions(project_path, provider, last_opened_at DESC);",
        )?;
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
        version = 2;
    }

    if version < 3 {
        let transaction = connection.transaction()?;
        // Early v2 installations required an opened timestamp even for sessions
        // merely discovered in a provider listing. SQLite needs a table rebuild
        // to remove that constraint; retain every label and timestamp as-is.
        let opened_required: bool = transaction.query_row(
            "SELECT \"notnull\" FROM pragma_table_info('sessions') WHERE name = 'last_opened_at'",
            [],
            |row| row.get(0),
        )?;
        if opened_required {
            transaction.execute_batch(
                "CREATE TABLE sessions_v3 (
                    project_path TEXT NOT NULL REFERENCES projects(path) ON DELETE CASCADE,
                    provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                    session_id TEXT NOT NULL,
                    first_seen_at INTEGER NOT NULL,
                    last_opened_at INTEGER,
                    local_label TEXT,
                    PRIMARY KEY (project_path, provider, session_id)
                 );
                 INSERT INTO sessions_v3
                    SELECT project_path, provider, session_id, first_seen_at, last_opened_at, local_label FROM sessions;
                 DROP TABLE sessions;
                 ALTER TABLE sessions_v3 RENAME TO sessions;
                 CREATE INDEX sessions_recent_idx
                    ON sessions(project_path, provider, last_opened_at DESC);",
            )?;
        }
        transaction.pragma_update(None, "user_version", 3)?;
        transaction.commit()?;
    }
    Ok(())
}

fn validate_provider(provider: &str) -> Result<(), DatabaseError> {
    if PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(DatabaseError::UnsupportedProvider(provider.to_owned()))
    }
}

fn validate_nonempty(field: &'static str, value: &str) -> Result<(), DatabaseError> {
    if value.trim().is_empty() {
        Err(DatabaseError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("could not create app data directory '{path}': {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("workspace database schema version {0} is newer than this application supports")]
    UnsupportedSchema(i64),
    #[error("workspace database contains an unsupported provider '{0}'")]
    UnsupportedProvider(String),
    #[error("project is not present in the workspace database: {0}")]
    UnknownProject(String),
    #[error("invalid workspace database value: {0}")]
    InvalidValue(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "caelum-database-test-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn database_path(&self) -> PathBuf {
            self.0.join("state").join("workspace.sqlite3")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn project(path: &str, name: &str) -> ProjectInfo {
        ProjectInfo {
            name: name.to_owned(),
            path: path.to_owned(),
        }
    }

    #[test]
    fn removing_a_session_clears_only_its_scoped_saved_selection() {
        let directory = TestDirectory::new("remove-session");
        let path = directory.database_path();
        let database = WorkspaceDatabase::open(&path).unwrap();
        for name in ["one", "two"] {
            let project_path = format!("/work/{name}");
            database
                .remember_project(&project(&project_path, name))
                .unwrap();
            for provider in PROVIDERS {
                database
                    .select_session(&project_path, provider, Some("same-id"))
                    .unwrap();
                database
                    .sync_sessions(&project_path, provider, &["same-id".into(), "kept".into()])
                    .unwrap();
            }
        }
        database
            .remove_session("/work/one", "codex", "same-id")
            .unwrap();
        database
            .remove_session("/work/one", "codex", "same-id")
            .unwrap();
        drop(database);
        let database = WorkspaceDatabase::open(&path).unwrap();
        for entry in database.restore().unwrap().projects {
            if entry.project.path == "/work/one" {
                assert_eq!(entry.codex_session_id, None);
                assert_eq!(entry.codex_session_ids, ["kept"]);
            } else {
                assert_eq!(entry.codex_session_id.as_deref(), Some("same-id"));
                assert_eq!(entry.codex_session_ids.len(), 2);
            }
            assert_eq!(entry.opencode_session_id.as_deref(), Some("same-id"));
            assert_eq!(entry.opencode_session_ids.len(), 2);
        }
    }

    #[test]
    fn fresh_database_migrates_and_can_be_opened_repeatedly() {
        let directory = TestDirectory::new("fresh");
        let database = WorkspaceDatabase::open(directory.database_path()).unwrap();
        assert!(database.restore().unwrap().projects.is_empty());
        drop(database);

        let database = WorkspaceDatabase::open(directory.database_path()).unwrap();
        assert!(database.restore().unwrap().projects.is_empty());
        let connection = Connection::open(directory.database_path()).unwrap();
        let version = connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn reports_unavailable_storage_without_panicking() {
        let directory = TestDirectory::new("unavailable");
        let blocking_file = directory.0.join("not-a-directory");
        fs::write(&blocking_file, "occupied").unwrap();

        let result = WorkspaceDatabase::open(blocking_file.join("workspace.sqlite3"));
        assert!(matches!(result, Err(DatabaseError::CreateDirectory { .. })));
    }

    #[test]
    fn version_one_database_upgrades_without_losing_projects() {
        let directory = TestDirectory::new("upgrade");
        let path = directory.database_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE projects (
                    path TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL,
                    is_open INTEGER NOT NULL DEFAULT 1 CHECK (is_open IN (0, 1)),
                    open_order INTEGER NOT NULL DEFAULT 0,
                    last_opened_at INTEGER NOT NULL,
                    provider TEXT NOT NULL DEFAULT 'codex' CHECK (provider IN ('codex', 'opencode'))
                 );
                 INSERT INTO projects (path, name, is_open, open_order, last_opened_at, provider)
                    VALUES ('/work/kept', 'kept', 1, 0, 10, 'opencode');
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let database = WorkspaceDatabase::open(&path).unwrap();
        let snapshot = database.restore().unwrap();
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].project.path, "/work/kept");
        assert_eq!(snapshot.projects[0].provider, "opencode");
        assert_eq!(snapshot.projects[0].opencode_session_id, None);
    }

    #[test]
    fn legacy_v2_session_timestamps_migrate_without_losing_metadata() {
        let directory = TestDirectory::new("legacy-v2");
        let path = directory.database_path();
        let database = WorkspaceDatabase::open(&path).unwrap();
        database
            .remember_project(&project("/work/kept", "kept"))
            .unwrap();
        database
            .select_session("/work/kept", "opencode", Some("ses_kept"))
            .unwrap();
        database.set_active_project(Some("/work/kept")).unwrap();
        database.lock().execute_batch(
            "DROP TABLE sessions;
             CREATE TABLE sessions (
                project_path TEXT NOT NULL REFERENCES projects(path) ON DELETE CASCADE,
                provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                session_id TEXT NOT NULL,
                first_seen_at INTEGER NOT NULL,
                last_opened_at INTEGER NOT NULL,
                local_label TEXT,
                PRIMARY KEY (project_path, provider, session_id)
             );
             CREATE INDEX sessions_recent_idx ON sessions(project_path, provider, last_opened_at DESC);
             INSERT INTO sessions VALUES ('/work/kept', 'opencode', 'ses_kept', 10, 20, 'Keep this label');
             PRAGMA user_version = 2;",
        ).unwrap();
        assert!(database
            .sync_sessions(
                "/work/kept",
                "opencode",
                &["ses_kept".into(), "ses_new".into()]
            )
            .is_err());
        drop(database);

        let database = WorkspaceDatabase::open(&path).unwrap();
        database
            .sync_sessions(
                "/work/kept",
                "opencode",
                &["ses_kept".into(), "ses_new".into()],
            )
            .unwrap();
        let snapshot = database.restore().unwrap();
        assert_eq!(snapshot.active_project_path.as_deref(), Some("/work/kept"));
        assert_eq!(
            snapshot.projects[0].opencode_session_id.as_deref(),
            Some("ses_kept")
        );
        let kept: (i64, i64, String) = database.lock().query_row(
            "SELECT first_seen_at, last_opened_at, local_label FROM sessions WHERE session_id = 'ses_kept'", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(kept, (10, 20, "Keep this label".into()));
        let opened: Option<i64> = database
            .lock()
            .query_row(
                "SELECT last_opened_at FROM sessions WHERE session_id = 'ses_new'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(opened, None);
        database
            .select_session("/work/kept", "opencode", Some("ses_new"))
            .unwrap();
        drop(database);
        let reopened = WorkspaceDatabase::open(&path).unwrap();
        assert_eq!(
            reopened.restore().unwrap().projects[0]
                .opencode_session_id
                .as_deref(),
            Some("ses_new")
        );
    }

    #[test]
    fn restores_open_projects_providers_and_selected_session_ids() {
        let directory = TestDirectory::new("round-trip");
        let database = WorkspaceDatabase::open(directory.database_path()).unwrap();
        database
            .remember_project(&project("/work/one", "one"))
            .unwrap();
        database
            .remember_project(&project("/work/two", "two"))
            .unwrap();
        database.set_provider("/work/one", "opencode").unwrap();
        database
            .select_session("/work/one", "codex", Some("thread-1"))
            .unwrap();
        database
            .select_session("/work/one", "opencode", Some("session-1"))
            .unwrap();
        database.set_active_project(Some("/work/one")).unwrap();

        let snapshot = database.restore().unwrap();
        assert_eq!(snapshot.active_project_path.as_deref(), Some("/work/one"));
        assert_eq!(snapshot.projects[0].project.path, "/work/two");
        let first = snapshot
            .projects
            .iter()
            .find(|entry| entry.project.path == "/work/one")
            .unwrap();
        assert_eq!(first.provider, "opencode");
        assert_eq!(first.codex_session_id.as_deref(), Some("thread-1"));
        assert_eq!(first.opencode_session_id.as_deref(), Some("session-1"));
        assert_eq!(first.codex_session_ids, ["thread-1"]);
        assert_eq!(first.opencode_session_ids, ["session-1"]);

        let connection = database.lock();
        let history_table_exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'conversation_history')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        assert!(!history_table_exists);
    }

    #[test]
    fn closing_project_keeps_session_metadata_but_removes_it_from_restore() {
        let directory = TestDirectory::new("close");
        let database = WorkspaceDatabase::open(directory.database_path()).unwrap();
        database
            .remember_project(&project("/work/one", "one"))
            .unwrap();
        database
            .remember_project(&project("/work/two", "two"))
            .unwrap();
        database
            .select_session("/work/one", "codex", Some("thread-1"))
            .unwrap();
        database.set_active_project(Some("/work/one")).unwrap();

        database
            .close_project("/work/one", Some("/work/two"))
            .unwrap();
        let snapshot = database.restore().unwrap();

        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.projects[0].project.path, "/work/two");
        assert_eq!(snapshot.active_project_path.as_deref(), Some("/work/two"));
        let connection = database.lock();
        let session_count = connection
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE project_path = '/work/one'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(session_count, 1);
    }

    #[test]
    fn remembers_provider_session_ids_without_persisting_conversation_content() {
        let directory = TestDirectory::new("session-inventory");
        let database = WorkspaceDatabase::open(directory.database_path()).unwrap();
        database
            .remember_project(&project("/work/one", "one"))
            .unwrap();
        database
            .sync_sessions(
                "/work/one",
                "codex",
                &["thread-1".into(), "thread-2".into()],
            )
            .unwrap();
        let restored = database.restore().unwrap();
        assert_eq!(
            restored.projects[0].codex_session_ids,
            ["thread-1", "thread-2"]
        );
        database
            .sync_sessions("/work/one", "codex", &["thread-2".into()])
            .unwrap();
        assert_eq!(
            database.restore().unwrap().projects[0].codex_session_ids,
            ["thread-2"]
        );

        let connection = database.lock();
        let (count, opened_count): (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*), COUNT(last_opened_at)
                 FROM sessions WHERE project_path = '/work/one' AND provider = 'codex'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(opened_count, 0);
        let history_table_exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'conversation_history')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        assert!(!history_table_exists);
    }
}

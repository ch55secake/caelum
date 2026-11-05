use rusqlite::Connection;

use super::DatabaseError;

pub(super) const CURRENT_SCHEMA_VERSION: i64 = 6;

pub(super) fn migrate(connection: &mut Connection) -> Result<(), DatabaseError> {
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
    if version < 4 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "ALTER TABLE projects ADD COLUMN root_path TEXT;
             ALTER TABLE projects ADD COLUMN selected_worktree_path TEXT;
             CREATE INDEX project_root_idx ON projects(root_path);",
        )?;
        transaction.pragma_update(None, "user_version", 4)?;
        transaction.commit()?;
    }
    if version < 5 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE session_branches (
                project_path TEXT NOT NULL,
                provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                session_id TEXT NOT NULL,
                branch TEXT,
                PRIMARY KEY (project_path, provider, session_id)
             );",
        )?;
        transaction.pragma_update(None, "user_version", 5)?;
        transaction.commit()?;
    }
    if version < 6 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE session_branch_history (
                project_path TEXT NOT NULL,
                provider TEXT NOT NULL CHECK (provider IN ('codex', 'opencode')),
                session_id TEXT NOT NULL,
                branch TEXT NOT NULL,
                PRIMARY KEY (project_path, provider, session_id, branch)
             );
             INSERT INTO session_branch_history
                SELECT project_path, provider, session_id, branch FROM session_branches
                WHERE branch IS NOT NULL;",
        )?;
        transaction.pragma_update(None, "user_version", 6)?;
        transaction.commit()?;
    }
    Ok(())
}

//! SQLite: connection and migrations (PLAN.md §3, stage 3).
//!
//! Migrations are applied in order; the schema version lives in `PRAGMA user_version`.
//! Never edit a published migration — append a new one.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const MIGRATIONS: &[&str] = &[
    // 1: clipboard history
    "CREATE TABLE clips (
        id          INTEGER PRIMARY KEY,
        kind        TEXT    NOT NULL CHECK (kind IN ('text', 'image')),
        text        TEXT,
        image_path  TEXT,
        hash        TEXT    NOT NULL UNIQUE,
        pinned      INTEGER NOT NULL DEFAULT 0,
        created_at  INTEGER NOT NULL,
        used_at     INTEGER NOT NULL
    );
    CREATE INDEX clips_order ON clips (pinned DESC, used_at DESC);",
];

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Internal(format!("ошибка базы данных: {err}"))
    }
}

/// Opens (creating if needed) the database at `path` and migrates it.
pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Applies every migration newer than the database's `user_version`, atomically.
pub fn migrate(conn: &mut Connection) -> AppResult<()> {
    let raw: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(raw).unwrap_or(usize::MAX);
    if current > MIGRATIONS.len() {
        return Err(AppError::Internal(format!(
            "база данных новее приложения (версия схемы {raw})"
        )));
    }
    let tx = conn.transaction()?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let version = i64::try_from(index + 1).unwrap_or(i64::MAX);
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        log::info!("database migrated to version {version}");
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrate(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> usize {
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        usize::try_from(v).unwrap()
    }

    #[test]
    fn migrates_a_fresh_database() {
        let conn = open_in_memory();
        assert_eq!(version(&conn), MIGRATIONS.len());
        conn.execute(
            "INSERT INTO clips (kind, text, hash, created_at, used_at) VALUES ('text', 'a', 'h', 1, 1)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn migration_is_idempotent_and_keeps_data() {
        let mut conn = open_in_memory();
        conn.execute(
            "INSERT INTO clips (kind, text, hash, created_at, used_at) VALUES ('text', 'keep', 'h', 1, 1)",
            [],
        )
        .unwrap();
        migrate(&mut conn).unwrap();
        let text: String = conn
            .query_row("SELECT text FROM clips", [], |row| row.get(0))
            .unwrap();
        assert_eq!(text, "keep");
        assert_eq!(version(&conn), MIGRATIONS.len());
    }

    #[test]
    fn opens_an_existing_file_database() {
        let path = std::env::temp_dir().join(format!("escape-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = open(&path).unwrap();
            conn.execute(
                "INSERT INTO clips (kind, text, hash, created_at, used_at) VALUES ('text', 'x', 'h', 1, 1)",
                [],
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM clips", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        drop(conn);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn refuses_a_newer_schema() {
        let mut conn = open_in_memory();
        conn.pragma_update(
            None,
            "user_version",
            i64::try_from(MIGRATIONS.len() + 1).unwrap(),
        )
        .unwrap();
        assert!(migrate(&mut conn).is_err());
    }

    #[test]
    fn rejects_unknown_clip_kinds() {
        let conn = open_in_memory();
        let result = conn.execute(
            "INSERT INTO clips (kind, hash, created_at, used_at) VALUES ('file', 'h', 1, 1)",
            [],
        );
        assert!(result.is_err());
    }
}

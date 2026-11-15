//! Notes with optional reminders in SQLite.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const MAX_TEXT_CHARS: usize = 500;

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub text: String,
    /// Unix ms; `None` = no reminder.
    pub remind_at: Option<i64>,
    /// The reminder already fired.
    pub reminded: bool,
    pub done: bool,
    pub created_at: i64,
}

const COLUMNS: &str = "id, text, remind_at, reminded, done, created_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        text: row.get(1)?,
        remind_at: row.get(2)?,
        reminded: row.get(3)?,
        done: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn clean_text(text: &str) -> AppResult<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::InvalidArgument("пустая заметка".into()));
    }
    Ok(text.chars().take(MAX_TEXT_CHARS).collect())
}

pub fn add(conn: &Connection, text: &str, remind_at: Option<i64>, now: i64) -> AppResult<Note> {
    let text = clean_text(text)?;
    conn.execute(
        "INSERT INTO notes (text, remind_at, created_at) VALUES (?1, ?2, ?3)",
        params![text, remind_at, now],
    )?;
    get(conn, conn.last_insert_rowid())?
        .ok_or_else(|| AppError::Internal("заметка пропала сразу после вставки".into()))
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<Note>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM notes WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

/// Open notes first (by reminder time, then newest), finished ones last.
pub fn list(conn: &Connection) -> AppResult<Vec<Note>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM notes
         ORDER BY done, remind_at IS NULL, remind_at, created_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn set_done(conn: &Connection, id: i64, done: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE notes SET done = ?1 WHERE id = ?2",
        params![done, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    Ok(())
}

/// Reminders that are due and have not fired yet; marks them as fired.
pub fn take_due(conn: &Connection, now: i64) -> AppResult<Vec<Note>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM notes
         WHERE reminded = 0 AND done = 0 AND remind_at IS NOT NULL AND remind_at <= ?1
         ORDER BY remind_at"
    ))?;
    let due = stmt
        .query_map([now], from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for note in &due {
        conn.execute("UPDATE notes SET reminded = 1 WHERE id = ?1", [note.id])?;
    }
    Ok(due
        .into_iter()
        .map(|n| Note {
            reminded: true,
            ..n
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    #[test]
    fn adds_lists_and_orders_notes() {
        let conn = open_in_memory();
        add(&conn, "без времени", None, 1).unwrap();
        add(&conn, "  позвонить маме в 18:30  ", Some(5_000), 2).unwrap();
        add(&conn, "раньше", Some(3_000), 3).unwrap();
        let done = add(&conn, "сделано", Some(1_000), 4).unwrap();
        set_done(&conn, done.id, true).unwrap();
        let texts: Vec<_> = list(&conn).unwrap().into_iter().map(|n| n.text).collect();
        assert_eq!(
            texts,
            ["раньше", "позвонить маме в 18:30", "без времени", "сделано"]
        );
    }

    #[test]
    fn rejects_empty_and_caps_long_text() {
        let conn = open_in_memory();
        assert!(add(&conn, "   ", None, 1).is_err());
        let long = add(&conn, &"а".repeat(900), None, 1).unwrap();
        assert_eq!(long.text.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn fires_each_due_reminder_once_and_skips_done_ones() {
        let conn = open_in_memory();
        let a = add(&conn, "a", Some(100), 1).unwrap();
        add(&conn, "later", Some(500), 1).unwrap();
        let b = add(&conn, "done", Some(100), 1).unwrap();
        set_done(&conn, b.id, true).unwrap();

        let due = take_due(&conn, 200).unwrap();
        assert_eq!(due.iter().map(|n| n.id).collect::<Vec<_>>(), [a.id]);
        assert!(due[0].reminded);
        assert!(take_due(&conn, 300).unwrap().is_empty());
        assert_eq!(take_due(&conn, 600).unwrap().len(), 1);
    }

    #[test]
    fn deletes() {
        let conn = open_in_memory();
        let n = add(&conn, "x", None, 1).unwrap();
        delete(&conn, n.id).unwrap();
        assert!(get(&conn, n.id).unwrap().is_none());
    }
}

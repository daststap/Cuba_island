//! Shelf entries in SQLite: references (paths) to files dropped on the island.

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
}

/// Adds paths; a path already on the shelf moves to the front instead of duplicating.
pub fn add(conn: &Connection, paths: &[String], now: i64) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO shelf (path, added_at) VALUES (?1, ?2)
         ON CONFLICT(path) DO UPDATE SET added_at = excluded.added_at",
    )?;
    for (offset, path) in paths.iter().enumerate() {
        // Keep the dropped order: later items get later timestamps.
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        stmt.execute(params![path, now.saturating_add(offset)])?;
    }
    Ok(())
}

/// Newest first.
pub fn list(conn: &Connection) -> AppResult<Vec<Entry>> {
    let mut stmt =
        conn.prepare("SELECT id, path, added_at FROM shelf ORDER BY added_at DESC, id DESC")?;
    let rows = stmt.query_map([], |row| {
        Ok(Entry {
            id: row.get(0)?,
            path: row.get(1)?,
            added_at: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, ids: &[i64]) -> AppResult<Vec<Entry>> {
    Ok(list(conn)?
        .into_iter()
        .filter(|entry| ids.contains(&entry.id))
        .collect())
}

pub fn remove(conn: &Connection, ids: &[i64]) -> AppResult<()> {
    let mut stmt = conn.prepare("DELETE FROM shelf WHERE id = ?1")?;
    for id in ids {
        stmt.execute([id])?;
    }
    Ok(())
}

pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM shelf", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    fn paths(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn adds_newest_first_and_dedupes() {
        let conn = open_in_memory();
        add(&conn, &["C:\\a.txt".into(), "C:\\b.png".into()], 10).unwrap();
        add(&conn, &["C:\\c".into()], 20).unwrap();
        assert_eq!(
            paths(&list(&conn).unwrap()),
            ["C:\\c", "C:\\b.png", "C:\\a.txt"]
        );

        add(&conn, &["C:\\a.txt".into()], 30).unwrap();
        assert_eq!(
            paths(&list(&conn).unwrap()),
            ["C:\\a.txt", "C:\\c", "C:\\b.png"]
        );
    }

    #[test]
    fn removes_selected_and_clears() {
        let conn = open_in_memory();
        add(&conn, &["a".into(), "b".into(), "c".into()], 1).unwrap();
        let all = list(&conn).unwrap();
        let ids: Vec<i64> = all.iter().filter(|e| e.path != "b").map(|e| e.id).collect();
        assert_eq!(paths(&get(&conn, &ids).unwrap()), ["c", "a"]);
        remove(&conn, &ids).unwrap();
        assert_eq!(paths(&list(&conn).unwrap()), ["b"]);
        clear(&conn).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}

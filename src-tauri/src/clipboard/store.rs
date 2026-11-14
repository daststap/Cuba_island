//! Clipboard history in SQLite: dedupe, pinning, search and the size limit.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::error::AppResult;

/// Longest text kept; bigger copies (whole files, logs) are skipped.
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Characters of text sent to the UI per item.
const PREVIEW_CHARS: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
}

impl ClipKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
        }
    }
}

/// A clip as stored. `text` is the full text; `image_path` is relative to the clips dir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub id: i64,
    pub kind: ClipKind,
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub used_at: i64,
}

/// A clip as sent to the frontend (mirrored in `src/ipc/commands.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub id: i64,
    pub kind: ClipKind,
    /// At most `PREVIEW_CHARS` characters.
    pub text: Option<String>,
    /// Absolute path of the 256 px preview (served through the asset protocol).
    pub thumb_path: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub used_at: i64,
}

impl Clip {
    pub fn to_item(&self, thumb_path: Option<String>) -> ClipItem {
        ClipItem {
            id: self.id,
            kind: self.kind,
            text: self
                .text
                .as_ref()
                .map(|t| t.chars().take(PREVIEW_CHARS).collect()),
            thumb_path,
            pinned: self.pinned,
            created_at: self.created_at,
            used_at: self.used_at,
        }
    }
}

pub enum NewClip<'a> {
    Text(&'a str),
    /// `path` of the saved PNG, relative to the clips dir.
    Image {
        path: &'a str,
    },
}

const COLUMNS: &str = "id, kind, text, image_path, pinned, created_at, used_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Clip> {
    let kind: String = row.get(1)?;
    Ok(Clip {
        id: row.get(0)?,
        kind: if kind == "image" {
            ClipKind::Image
        } else {
            ClipKind::Text
        },
        text: row.get(2)?,
        image_path: row.get(3)?,
        pinned: row.get(4)?,
        created_at: row.get(5)?,
        used_at: row.get(6)?,
    })
}

/// Stable content hash for dedupe: 64-bit FNV-1a plus kind and length.
pub fn content_hash(kind: ClipKind, bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{}:{}:{hash:016x}", kind.as_str(), bytes.len())
}

/// Inserts a clip, or — if the same content exists — moves it to the top.
/// Returns the stored clip and whether it was new.
pub fn upsert(
    conn: &Connection,
    clip: NewClip<'_>,
    hash: &str,
    now: i64,
) -> AppResult<(Clip, bool)> {
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM clips WHERE hash = ?1", [hash], |row| {
            row.get(0)
        })
        .optional()?;
    let id = if let Some(id) = existing {
        conn.execute(
            "UPDATE clips SET used_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        id
    } else {
        let (kind, text, image_path) = match clip {
            NewClip::Text(text) => (ClipKind::Text, Some(text), None),
            NewClip::Image { path } => (ClipKind::Image, None, Some(path)),
        };
        conn.execute(
            "INSERT INTO clips (kind, text, image_path, hash, created_at, used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![kind.as_str(), text, image_path, hash, now],
        )?;
        conn.last_insert_rowid()
    };
    let clip = get(conn, id)?.ok_or_else(|| {
        crate::error::AppError::Internal("запись пропала сразу после вставки".into())
    })?;
    Ok((clip, existing.is_none()))
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<Clip>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM clips WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

/// Pinned first, then most recently used. `query` filters text clips,
/// case-insensitively for any script (SQLite's LIKE only folds ASCII).
pub fn list(
    conn: &Connection,
    query: Option<&str>,
    limit: usize,
    offset: usize,
) -> AppResult<Vec<Clip>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM clips ORDER BY pinned DESC, used_at DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    let needle = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);

    let mut out = Vec::new();
    let mut skipped = 0;
    for row in rows {
        let clip = row?;
        if let Some(needle) = &needle {
            let matches = clip
                .text
                .as_ref()
                .is_some_and(|t| t.to_lowercase().contains(needle.as_str()));
            if !matches {
                continue;
            }
        }
        if skipped < offset {
            skipped += 1;
            continue;
        }
        out.push(clip);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

pub fn touch(conn: &Connection, id: i64, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE clips SET used_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE clips SET pinned = ?1 WHERE id = ?2",
        params![pinned, id],
    )?;
    Ok(())
}

/// Deletes a clip; returns its image path (if any) so the caller can remove the files.
pub fn delete(conn: &Connection, id: i64) -> AppResult<Option<String>> {
    let path: Option<Option<String>> = conn
        .query_row("SELECT image_path FROM clips WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    Ok(path.flatten())
}

/// Deletes every unpinned clip; returns their image paths.
pub fn clear(conn: &Connection) -> AppResult<Vec<String>> {
    delete_where(conn, "pinned = 0", [])
}

/// Keeps at most `limit` unpinned clips (pinned ones never count or go);
/// returns image paths of the removed clips.
pub fn enforce_limit(conn: &Connection, limit: usize) -> AppResult<Vec<String>> {
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    delete_where(
        conn,
        "pinned = 0 AND id NOT IN (
            SELECT id FROM clips WHERE pinned = 0 ORDER BY used_at DESC, id DESC LIMIT ?1)",
        [limit],
    )
}

fn delete_where<P: rusqlite::Params + Copy>(
    conn: &Connection,
    condition: &str,
    params: P,
) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT image_path FROM clips WHERE image_path IS NOT NULL AND {condition}"
    ))?;
    let paths = stmt
        .query_map(params, |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    conn.execute(&format!("DELETE FROM clips WHERE {condition}"), params)?;
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory;

    fn add_text(conn: &Connection, text: &str, now: i64) -> (Clip, bool) {
        let hash = content_hash(ClipKind::Text, text.as_bytes());
        upsert(conn, NewClip::Text(text), &hash, now).unwrap()
    }

    fn texts(clips: &[Clip]) -> Vec<&str> {
        clips.iter().filter_map(|c| c.text.as_deref()).collect()
    }

    #[test]
    fn dedupes_and_moves_repeated_copies_to_the_top() {
        let conn = open_in_memory();
        let (first, new) = add_text(&conn, "a", 1);
        assert!(new);
        add_text(&conn, "b", 2);
        let (again, new) = add_text(&conn, "a", 3);
        assert!(!new);
        assert_eq!(again.id, first.id);
        assert_eq!(again.created_at, 1);
        assert_eq!(texts(&list(&conn, None, 10, 0).unwrap()), ["a", "b"]);
    }

    #[test]
    fn pinned_clips_come_first_and_survive_limit_and_clear() {
        let conn = open_in_memory();
        let (pinned, _) = add_text(&conn, "pinned", 1);
        set_pinned(&conn, pinned.id, true).unwrap();
        for i in 0..5 {
            add_text(&conn, &format!("t{i}"), 10 + i);
        }
        enforce_limit(&conn, 2).unwrap();
        assert_eq!(
            texts(&list(&conn, None, 10, 0).unwrap()),
            ["pinned", "t4", "t3"]
        );
        clear(&conn).unwrap();
        assert_eq!(texts(&list(&conn, None, 10, 0).unwrap()), ["pinned"]);
    }

    #[test]
    fn limit_returns_image_paths_to_delete() {
        let conn = open_in_memory();
        let hash = content_hash(ClipKind::Image, b"png");
        upsert(&conn, NewClip::Image { path: "old.png" }, &hash, 1).unwrap();
        add_text(&conn, "new", 2);
        assert_eq!(enforce_limit(&conn, 1).unwrap(), ["old.png"]);
        assert_eq!(texts(&list(&conn, None, 10, 0).unwrap()), ["new"]);
    }

    #[test]
    fn searches_case_insensitively_in_any_script() {
        let conn = open_in_memory();
        add_text(&conn, "Привет, Мир", 1);
        add_text(&conn, "hello WORLD", 2);
        add_text(&conn, "другое", 3);
        assert_eq!(
            texts(&list(&conn, Some("мир"), 10, 0).unwrap()),
            ["Привет, Мир"]
        );
        assert_eq!(
            texts(&list(&conn, Some(" world "), 10, 0).unwrap()),
            ["hello WORLD"]
        );
        assert_eq!(list(&conn, Some(""), 10, 0).unwrap().len(), 3);
    }

    #[test]
    fn paginates() {
        let conn = open_in_memory();
        for i in 0..5 {
            add_text(&conn, &format!("t{i}"), i);
        }
        assert_eq!(texts(&list(&conn, None, 2, 1).unwrap()), ["t3", "t2"]);
    }

    #[test]
    fn deletes_and_touches() {
        let conn = open_in_memory();
        let (a, _) = add_text(&conn, "a", 1);
        let (b, _) = add_text(&conn, "b", 2);
        touch(&conn, a.id, 3).unwrap();
        assert_eq!(texts(&list(&conn, None, 10, 0).unwrap()), ["a", "b"]);
        assert_eq!(delete(&conn, b.id).unwrap(), None);
        assert!(get(&conn, b.id).unwrap().is_none());
    }

    #[test]
    fn hash_distinguishes_kind_and_content() {
        let t = content_hash(ClipKind::Text, b"abc");
        assert_eq!(t, content_hash(ClipKind::Text, b"abc"));
        assert_ne!(t, content_hash(ClipKind::Image, b"abc"));
        assert_ne!(t, content_hash(ClipKind::Text, b"abd"));
    }

    #[test]
    fn preview_is_truncated() {
        let conn = open_in_memory();
        let long = "я".repeat(1000);
        let (clip, _) = add_text(&conn, &long, 1);
        let item = clip.to_item(None);
        assert_eq!(item.text.unwrap().chars().count(), PREVIEW_CHARS);
    }
}

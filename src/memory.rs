//! SQLite-backed memory store (sessions + episodic notes).
//!
//! Two tables:
//!
//! * `sessions` — one row per user session (chat id, channel, created).
//! * `notes` — key/value episodic notes shared across sessions.

use std::path::Path;
use std::sync::Mutex;

use crate::error::Result;

/// In-process memory store.
#[derive(Debug)]
pub struct Memory {
    conn: Mutex<rusqlite::Connection>,
}

impl Memory {
    /// Open an in-memory database (handy for tests).
    pub fn in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Self::init(conn)
    }

    /// Open a file-backed database; pass `None` for `:memory:`.
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let conn = match path {
            Some(p) => {
                if let Some(parent) = p.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
                rusqlite::Connection::open(p)?
            }
            None => rusqlite::Connection::open_in_memory()?,
        };
        Self::init(conn)
    }

    fn init(conn: rusqlite::Connection) -> Result<Self> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id         TEXT PRIMARY KEY,
                channel    TEXT NOT NULL,
                chat_id    TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS notes (
                key        TEXT PRIMARY KEY,
                value      TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create or reset a session row.
    pub fn upsert_session(&self, id: &str, channel: &str, chat_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, channel, chat_id) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET channel=excluded.channel, chat_id=excluded.chat_id",
            rusqlite::params![id, channel, chat_id],
        )?;
        Ok(())
    }

    /// Write a note (overwriting).
    pub fn write_note(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO notes (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Read a note by key.
    pub fn read_note(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT value FROM notes WHERE key = ?1")
            .map_err(|e| anyhow::anyhow!("prepare: {e}"))?;
        let mut rows = stmt
            .query(rusqlite::params![key])
            .map_err(|e| anyhow::anyhow!("query: {e}"))?;
        if let Some(row) = rows.next().map_err(|e| anyhow::anyhow!("row: {e}"))? {
            let v: String = row.get(0).map_err(|e| anyhow::anyhow!("get: {e}"))?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    /// Delete a note by key. Returns whether something was deleted.
    pub fn delete_note(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM notes WHERE key = ?1", rusqlite::params![key])?;
        Ok(n > 0)
    }

    /// List all `(key, value)` note pairs.
    pub fn list_notes(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT key, value FROM notes ORDER BY key")
            .map_err(|e| anyhow::anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| anyhow::anyhow!("query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (k, v): (String, String) = row.map_err(|e| anyhow::anyhow!("row: {e}"))?;
            out.push((k, v));
        }
        Ok(out)
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, channel, chat_id, created_at FROM sessions ORDER BY created_at DESC",
            )
            .map_err(|e| anyhow::anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SessionRow {
                    id: r.get(0)?,
                    channel: r.get(1)?,
                    chat_id: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })
            .map_err(|e| anyhow::anyhow!("query: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| anyhow::anyhow!("row: {e}"))?);
        }
        Ok(out)
    }
}

/// A session row.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub channel: String,
    pub chat_id: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_round_trip() {
        let m = Memory::in_memory().unwrap();
        m.write_note("k", "v").unwrap();
        assert_eq!(m.read_note("k").unwrap().as_deref(), Some("v"));
        assert!(m.read_note("missing").unwrap().is_none());
        assert!(m.delete_note("k").unwrap());
        assert!(!m.delete_note("k").unwrap());
    }

    #[test]
    fn list_notes_sorted() {
        let m = Memory::in_memory().unwrap();
        m.write_note("b", "2").unwrap();
        m.write_note("a", "1").unwrap();
        let notes = m.list_notes().unwrap();
        assert_eq!(
            notes,
            vec![("a".into(), "1".into()), ("b".into(), "2".into())]
        );
    }

    #[test]
    fn sessions_round_trip() {
        let m = Memory::in_memory().unwrap();
        m.upsert_session("s1", "telegram", "42").unwrap();
        m.upsert_session("s2", "telegram", "42").unwrap();
        let list = m.list_sessions().unwrap();
        assert_eq!(list.len(), 2);
    }
}

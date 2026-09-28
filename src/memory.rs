//! SQLite-backed memory store (sessions + episodic notes).
//!
//! Two tables:
//!
//! * `sessions` — one row per user session (chat id, channel, created).
//! * `notes` — key/value episodic notes shared across sessions.

use std::path::Path;
use std::sync::Mutex;

use crate::agent::Message;
use crate::error::Result;

/// In-process memory store.
#[derive(Debug)]
pub struct Memory {
    conn: Mutex<rusqlite::Connection>,
}

impl Memory {
    /// Take the connection lock, recovering from poisoning.
    ///
    /// A poisoned mutex means some other caller panicked mid-statement; the
    /// SQLite connection itself is still usable, and every method here runs
    /// a single self-contained statement. `unwrap()` would be worse than
    /// unhelpful — the release profile sets `panic = "abort"`, so one
    /// poisoned lock would kill the whole gateway.
    fn lock(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("memory mutex was poisoned; recovering");
            poisoned.into_inner()
        })
    }

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
            CREATE TABLE IF NOT EXISTS messages (
                session_id TEXT NOT NULL,
                position   INTEGER NOT NULL,
                payload    TEXT NOT NULL,
                text       TEXT NOT NULL,
                PRIMARY KEY (session_id, position)
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS message_search USING fts5(
                session_id UNINDEXED,
                position UNINDEXED,
                text
            );
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create or reset a session row.
    pub fn upsert_session(&self, id: &str, channel: &str, chat_id: &str) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO sessions (id, channel, chat_id) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET channel=excluded.channel, chat_id=excluded.chat_id",
            rusqlite::params![id, channel, chat_id],
        )?;
        Ok(())
    }

    /// Write a note (overwriting).
    pub fn write_note(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO notes (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Read a note by key.
    pub fn read_note(&self, key: &str) -> Result<Option<String>> {
        let conn = self.lock();
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
        let conn = self.lock();
        let n = conn.execute("DELETE FROM notes WHERE key = ?1", rusqlite::params![key])?;
        Ok(n > 0)
    }

    /// List all `(key, value)` note pairs.
    pub fn list_notes(&self) -> Result<Vec<(String, String)>> {
        let conn = self.lock();
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

    /// List sessions, newest first, capped at `limit` rows.
    /// Defaults to 1000 to bound memory growth on long-running gateways.
    pub fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        self.list_sessions_limited(1000)
    }

    /// List sessions with an explicit row cap.
    pub fn list_sessions_limited(&self, limit: usize) -> Result<Vec<SessionRow>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, channel, chat_id, created_at FROM sessions \
                 ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| anyhow::anyhow!("prepare: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![limit as i64], |r| {
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

    /// Replace a session transcript atomically. Storing the full message JSON
    /// preserves tool-use/result pairing across restarts; a separate FTS5
    /// index supports low-cost cross-session recall.
    pub fn save_history(&self, session_id: &str, history: &[Message]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        tx.execute(
            "DELETE FROM message_search WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        for (position, message) in history.iter().enumerate() {
            let payload = serde_json::to_string(message)?;
            let text = message.text();
            tx.execute(
                "INSERT INTO messages (session_id, position, payload, text) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![session_id, position as i64, payload, text],
            )?;
            tx.execute(
                "INSERT INTO message_search (session_id, position, text) VALUES (?1, ?2, ?3)",
                rusqlite::params![session_id, position as i64, message.text()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Restore a durable session transcript, or an empty history when it has
    /// never been seen before.
    pub fn load_history(&self, session_id: &str) -> Result<Vec<Message>> {
        let conn = self.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT payload FROM messages WHERE session_id = ?1 ORDER BY position",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str::<Message>(&row?)?))
            .collect()
    }

    /// Full-text recall across all durable sessions. Invalid FTS query syntax
    /// falls back to quoted text so ordinary user messages remain searchable.
    pub fn search_history(&self, query: &str, limit: usize) -> Result<Vec<RecallHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT session_id, position, text FROM message_search WHERE message_search MATCH ?1 LIMIT ?2",
        )?;
        let escaped = query.replace('"', " ");
        let rows = stmt.query_map(
            rusqlite::params![format!("\"{escaped}\""), limit as i64],
            |r| {
                Ok(RecallHit {
                    session_id: r.get(0)?,
                    position: r.get::<_, i64>(1)? as usize,
                    text: r.get(2)?,
                })
            },
        )?;
        rows.map(|row| row.map_err(Into::into)).collect()
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

/// A full-text recall result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallHit {
    /// Owning persistent session.
    pub session_id: String,
    /// Message position within that session.
    pub position: usize,
    /// Searchable text, suitable for prompt-grounded recall.
    pub text: String,
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

    #[test]
    fn histories_are_durable_and_searchable() {
        let m = Memory::in_memory().unwrap();
        let history = vec![
            Message::user("I prefer compact Rust reviews"),
            Message::assistant_text("noted"),
        ];
        m.save_history("telegram-42", &history).unwrap();
        assert_eq!(
            m.load_history("telegram-42").unwrap()[0].text(),
            history[0].text()
        );
        let hits = m.search_history("compact Rust", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "telegram-42");
    }
}

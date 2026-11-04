//! Matrix channel (client-server API, plain HTTPS).
//!
//! Matrix needs no WebSocket: `GET /sync?since=…&timeout=…` is a long poll
//! exactly like Telegram's `getUpdates`, and replies are
//! `PUT /rooms/{room}/send/m.room.message/{txn}`. That keeps the dependency
//! footprint at zero — it reuses `reqwest` — which is why Matrix, rather than a
//! WebSocket-only platform, is the second gateway.
//!
//! Trust model: the agent has a shell, so only messages from an allowed room
//! or an allowed user are dispatched (see [`MatrixConfig::allows`]); everything
//! else is dropped before it reaches the model.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::channels::{parse_command, Channel, InboundMessage};
use crate::error::Result;
use crate::util::{chunk_text, random_hex, truncate_bytes};

/// Longest backoff between failed `/sync` polls.
const MAX_SYNC_BACKOFF: Duration = Duration::from_secs(60);
/// Replies are split below this many characters per event.
pub const MATRIX_MAX_MESSAGE_CHARS: usize = 8000;

/// Matrix channel configuration.
#[derive(Debug, Clone)]
pub struct MatrixConfig {
    /// Homeserver base URL, e.g. `https://matrix.example.org`.
    pub homeserver: String,
    /// Access token of the bot account.
    pub access_token: String,
    /// Rooms (`!id:server`) the bot serves.
    pub allowed_rooms: Vec<String>,
    /// Users (`@name:server`) the bot serves in any room.
    pub allowed_users: Vec<String>,
    /// Serve every room and user (explicit opt-in, like Telegram's).
    pub allow_all: bool,
    /// Long-poll timeout in seconds.
    pub poll_timeout: u32,
}

impl MatrixConfig {
    /// May a message from `sender` in `room` reach the agent?
    pub fn allows(&self, room: &str, sender: &str) -> bool {
        self.allow_all
            || self.allowed_rooms.iter().any(|r| r == room)
            || self.allowed_users.iter().any(|u| u == sender)
    }

    fn base(&self) -> String {
        format!(
            "{}/_matrix/client/v3",
            self.homeserver.trim_end_matches('/')
        )
    }
}

/// Percent-encode a path segment (room ids contain `!` and `:`).
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// What one `/sync` response contributes.
#[derive(Debug, Default)]
pub struct SyncBatch {
    /// Token for the next poll.
    pub next_batch: Option<String>,
    /// Allowed, non-self text messages.
    pub messages: Vec<InboundMessage>,
    /// Rooms to join (invites from an allowed room/user).
    pub joins: Vec<String>,
}

/// Extract messages and trusted invites from a `/sync` body. Pure, so the
/// allow-list and self-message rules are unit-testable.
pub fn parse_sync(body: &Value, cfg: &MatrixConfig, own_user: &str) -> SyncBatch {
    let mut batch = SyncBatch {
        next_batch: body
            .get("next_batch")
            .and_then(Value::as_str)
            .map(str::to_string),
        ..SyncBatch::default()
    };
    if let Some(rooms) = body.pointer("/rooms/join").and_then(Value::as_object) {
        for (room, data) in rooms {
            let events = data
                .pointer("/timeline/events")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for ev in events {
                if ev.get("type").and_then(Value::as_str) != Some("m.room.message") {
                    continue;
                }
                let sender = ev.get("sender").and_then(Value::as_str).unwrap_or("");
                if sender.is_empty() || sender == own_user || !cfg.allows(room, sender) {
                    continue;
                }
                let msgtype = ev.pointer("/content/msgtype").and_then(Value::as_str);
                if !matches!(msgtype, Some("m.text") | Some("m.notice") | None) {
                    continue;
                }
                let Some(text) = ev.pointer("/content/body").and_then(Value::as_str) else {
                    continue;
                };
                let ts = ev
                    .get("origin_server_ts")
                    .and_then(Value::as_i64)
                    .and_then(chrono::DateTime::from_timestamp_millis)
                    .unwrap_or_else(chrono::Utc::now);
                let (command, args) = parse_command(text);
                batch.messages.push(InboundMessage {
                    channel: "matrix".into(),
                    chat_id: room.clone(),
                    user_id: Some(sender.to_string()),
                    user_name: Some(sender.to_string()),
                    timestamp: ts,
                    text: text.to_string(),
                    command,
                    args,
                });
            }
        }
    }
    if let Some(invites) = body.pointer("/rooms/invite").and_then(Value::as_object) {
        for (room, data) in invites {
            let inviter = data
                .pointer("/invite_state/events")
                .and_then(Value::as_array)
                .and_then(|evs| {
                    evs.iter().find(|e| {
                        e.get("type").and_then(Value::as_str) == Some("m.room.member")
                            && e.get("state_key").and_then(Value::as_str) == Some(own_user)
                    })
                })
                .and_then(|e| e.get("sender").and_then(Value::as_str))
                .unwrap_or("");
            if !inviter.is_empty() && cfg.allows(room, inviter) {
                batch.joins.push(room.clone());
            }
        }
    }
    batch
}

/// Matrix channel.
pub struct MatrixChannel {
    cfg: MatrixConfig,
    client: reqwest::Client,
}

impl MatrixChannel {
    /// Build a channel from a config.
    pub fn new(cfg: MatrixConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.poll_timeout as u64 + 30))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "matrix client build failed; using default client");
                reqwest::Client::new()
            });
        Self { cfg, client }
    }

    async fn whoami(&self) -> Result<String> {
        let resp = self
            .client
            .get(format!("{}/account/whoami", self.cfg.base()))
            .bearer_auth(&self.cfg.access_token)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("matrix whoami: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if matches!(status.as_u16(), 401 | 403) {
            anyhow::bail!(
                "matrix rejected the access token ({status}): {}. Check `matrix.access_token`.",
                truncate_bytes(&body, 256)
            );
        }
        if !status.is_success() {
            anyhow::bail!("matrix whoami {status}: {}", truncate_bytes(&body, 256));
        }
        let v: Value =
            serde_json::from_str(&body).map_err(|e| anyhow::anyhow!("matrix whoami parse: {e}"))?;
        v.get("user_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("matrix whoami: no user_id"))
    }

    async fn join(&self, room: &str) {
        let url = format!("{}/join/{}", self.cfg.base(), encode_segment(room));
        match self
            .client
            .post(url)
            .bearer_auth(&self.cfg.access_token)
            .json(&json!({}))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => tracing::info!(room, "matrix: joined room"),
            Ok(r) => tracing::warn!(room, status = %r.status(), "matrix: join refused"),
            Err(e) => tracing::warn!(room, error = %e, "matrix: join failed"),
        }
    }
}

#[async_trait]
impl Channel for MatrixChannel {
    fn name(&self) -> &str {
        "matrix"
    }

    async fn run(&self, tx: tokio::sync::mpsc::Sender<InboundMessage>) -> Result<()> {
        let own_user = self.whoami().await?;
        // The first sync returns the whole backlog; take its token but do not
        // dispatch it, or every restart would replay old messages to the model.
        let mut since: Option<String> = None;
        let mut backoff = Duration::from_secs(2);
        loop {
            let first = since.is_none();
            let timeout_ms = if first {
                0
            } else {
                self.cfg.poll_timeout as u64 * 1000
            };
            let mut url = format!("{}/sync?timeout={timeout_ms}", self.cfg.base());
            if let Some(s) = &since {
                url.push_str(&format!("&since={}", encode_segment(s)));
            }
            let resp = match self
                .client
                .get(&url)
                .bearer_auth(&self.cfg.access_token)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, backoff = ?backoff, "matrix sync failed");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_SYNC_BACKOFF);
                    continue;
                }
            };
            let status = resp.status();
            if matches!(status.as_u16(), 401 | 403) {
                anyhow::bail!("matrix rejected the access token ({status})");
            }
            if !status.is_success() {
                tracing::warn!(%status, backoff = ?backoff, "matrix sync returned an error");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_SYNC_BACKOFF);
                continue;
            }
            let body: Value = match resp.json().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "matrix sync parse");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_SYNC_BACKOFF);
                    continue;
                }
            };
            backoff = Duration::from_secs(2);
            let batch = parse_sync(&body, &self.cfg, &own_user);
            for room in &batch.joins {
                self.join(room).await;
            }
            if !first {
                for m in batch.messages {
                    if tx.send(m).await.is_err() {
                        return Ok(()); // gateway shut down
                    }
                }
            }
            match batch.next_batch {
                Some(next) => since = Some(next),
                None => {
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    async fn send(&self, chat_id: &str, text: &str) -> Result<()> {
        for chunk in chunk_text(text, MATRIX_MAX_MESSAGE_CHARS) {
            if chunk.trim().is_empty() {
                continue;
            }
            let txn = random_hex(8);
            let url = format!(
                "{}/rooms/{}/send/m.room.message/{txn}",
                self.cfg.base(),
                encode_segment(chat_id)
            );
            let resp = self
                .client
                .put(url)
                .bearer_auth(&self.cfg.access_token)
                .json(&json!({"msgtype": "m.text", "body": chunk}))
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("matrix send: {e}"))?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("matrix send {status}: {}", truncate_bytes(&body, 256));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MatrixConfig {
        MatrixConfig {
            homeserver: "https://hs".into(),
            access_token: "t".into(),
            allowed_rooms: vec!["!ok:hs".into()],
            allowed_users: vec!["@boss:hs".into()],
            allow_all: false,
            poll_timeout: 30,
        }
    }

    fn msg(sender: &str, body: &str) -> Value {
        json!({"type":"m.room.message","sender":sender,"origin_server_ts":1_700_000_000_000i64,
               "content":{"msgtype":"m.text","body":body}})
    }

    #[test]
    fn parse_sync_applies_allow_list_and_skips_own_messages() {
        let body = json!({"next_batch":"s2","rooms":{"join":{
            "!ok:hs":   {"timeline":{"events":[msg("@anyone:hs","hello"), msg("@bot:hs","my own"), msg("@anyone:hs","/new now")]}},
            "!other:hs":{"timeline":{"events":[msg("@stranger:hs","let me in"), msg("@boss:hs","from boss")]}}
        }}});
        let b = parse_sync(&body, &cfg(), "@bot:hs");
        assert_eq!(b.next_batch.as_deref(), Some("s2"));
        let texts: Vec<_> = b.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(
            texts.contains(&"hello") && texts.contains(&"/new now") && texts.contains(&"from boss")
        );
        assert!(
            !texts.contains(&"my own") && !texts.contains(&"let me in"),
            "{texts:?}"
        );
        let cmd = b.messages.iter().find(|m| m.text == "/new now").unwrap();
        assert_eq!(cmd.command.as_deref(), Some("new"));
        assert_eq!(cmd.channel, "matrix");
        assert_eq!(cmd.chat_id, "!ok:hs");
    }

    #[test]
    fn parse_sync_joins_only_trusted_invites_and_ignores_non_text() {
        let invite = |room: &str, from: &str| {
            json!({room: {"invite_state":{"events":[
                {"type":"m.room.member","state_key":"@bot:hs","sender":from,"content":{"membership":"invite"}}]}}})
        };
        let mut rooms = serde_json::Map::new();
        for (r, f) in [
            ("!x:hs", "@boss:hs"),
            ("!y:hs", "@stranger:hs"),
            ("!ok:hs", "@stranger:hs"),
        ] {
            rooms.extend(invite(r, f).as_object().unwrap().clone());
        }
        let body = json!({"rooms":{"invite":rooms,"join":{"!ok:hs":{"timeline":{"events":[
            {"type":"m.room.message","sender":"@a:hs","content":{"msgtype":"m.image","body":"pic.png"}},
            {"type":"m.room.member","sender":"@a:hs","content":{}}]}}}}});
        let mut b = parse_sync(&body, &cfg(), "@bot:hs");
        b.joins.sort();
        assert_eq!(b.joins, vec!["!ok:hs".to_string(), "!x:hs".to_string()]);
        assert!(b.messages.is_empty());
        assert!(b.next_batch.is_none());
    }

    #[test]
    fn allow_all_and_encoding() {
        let mut c = cfg();
        assert!(!c.allows("!z:hs", "@z:hs"));
        c.allow_all = true;
        assert!(c.allows("!z:hs", "@z:hs"));
        assert_eq!(encode_segment("!a b:hs"), "%21a%20b%3Ahs");
    }
}

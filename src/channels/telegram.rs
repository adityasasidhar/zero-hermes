//! Telegram channel.
//!
//! Uses the long-poll `getUpdates` endpoint and `sendMessage`. The
//! implementation is intentionally minimal: poll, dispatch, repeat.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::channels::{parse_command, strip_bot_mention, Channel, InboundMessage};
use crate::error::Result;

/// Telegram channel configuration.
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    pub token: String,
    pub poll_timeout: u32,
    pub allowed_chats: Vec<i64>,
    pub allowed_commands: Vec<String>,
}

impl TelegramConfig {
    fn base_url(&self) -> String {
        format!("https://api.telegram.org/bot{}", self.token)
    }
}

/// Telegram channel.
pub struct TelegramChannel {
    cfg: TelegramConfig,
    client: reqwest::Client,
}

impl TelegramChannel {
    /// Build a channel from a config.
    pub fn new(cfg: TelegramConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client");
        Self { cfg, client }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn run(&self, tx: tokio::sync::mpsc::Sender<InboundMessage>) -> Result<()> {
        let mut offset: Option<i64> = None;
        loop {
            let mut url = format!(
                "{}/getUpdates?timeout={}",
                self.cfg.base_url(),
                self.cfg.poll_timeout
            );
            if let Some(off) = offset {
                url.push_str(&format!("&offset={off}"));
            }
            let resp = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "telegram getUpdates failed");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            let body: UpdateResponse = match resp.json().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "telegram getUpdates parse");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };

            for upd in body.result {
                if let Some(msg) = upd.message {
                    if let Some(text) = msg.text {
                        let chat_id = msg.chat.id.to_string();
                        if !self.cfg.allowed_chats.is_empty()
                            && !self.cfg.allowed_chats.contains(&msg.chat.id)
                        {
                            offset = Some(upd.update_id + 1);
                            continue;
                        }
                        let (cmd, args) = parse_command(&text);
                        let cmd = cmd.map(|c| strip_bot_mention(&c));
                        if let Some(c) = &cmd {
                            if !self.cfg.allowed_commands.is_empty()
                                && !self.cfg.allowed_commands.iter().any(|a| a == c)
                            {
                                offset = Some(upd.update_id + 1);
                                continue;
                            }
                        }
                        let inbound = InboundMessage {
                            channel: "telegram".to_string(),
                            chat_id,
                            user_id: msg.from.as_ref().map(|u| u.id.to_string()),
                            user_name: msg.from.as_ref().map(|u| {
                                u.username.clone().unwrap_or_else(|| u.first_name.clone())
                            }),
                            timestamp: chrono::Utc::now(),
                            text: text.clone(),
                            command: cmd,
                            args,
                        };
                        if tx.send(inbound).await.is_err() {
                            tracing::info!("inbound channel closed");
                            return Ok(());
                        }
                    }
                }
                offset = Some(upd.update_id + 1);
            }
        }
    }

    async fn send(&self, chat_id: &str, text: &str) -> Result<()> {
        let url = format!("{}/sendMessage", self.cfg.base_url());
        let payload = SendMessagePayload {
            chat_id,
            text,
            parse_mode: None,
        };
        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("telegram send: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "telegram send {}: {}",
                status,
                truncate(&body, 256)
            ));
        }
        Ok(())
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut t = s[..max].to_string();
        t.push('…');
        t
    }
}

#[derive(Debug, Deserialize)]
struct UpdateResponse {
    #[allow(dead_code)]
    ok: bool,
    result: Vec<Update>,
}

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    #[serde(default)]
    message: Option<UpdateMessage>,
}

#[derive(Debug, Deserialize)]
struct UpdateMessage {
    #[allow(dead_code)]
    message_id: i64,
    #[serde(default)]
    from: Option<UpdateUser>,
    chat: UpdateChat,
    #[serde(default)]
    text: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    date: i64,
}

#[derive(Debug, Deserialize)]
struct UpdateUser {
    id: i64,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    first_name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateChat {
    id: i64,
}

#[derive(Debug, Serialize)]
struct SendMessagePayload<'a> {
    chat_id: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::parse_command;

    #[test]
    fn build_config_url() {
        let cfg = TelegramConfig {
            token: "123:abc".to_string(),
            poll_timeout: 30,
            allowed_chats: Vec::new(),
            allowed_commands: Vec::new(),
        };
        assert!(cfg.base_url().contains("123:abc"));
    }

    #[test]
    fn channel_name() {
        let ch = TelegramChannel::new(TelegramConfig {
            token: "t".into(),
            poll_timeout: 30,
            allowed_chats: Vec::new(),
            allowed_commands: Vec::new(),
        });
        assert_eq!(ch.name(), "telegram");
    }

    #[test]
    fn parse_command_works_for_telegram() {
        let (cmd, args) = parse_command("/ping@mybot hello");
        assert_eq!(cmd.as_deref(), Some("ping@mybot"));
        assert_eq!(super::strip_bot_mention(cmd.as_deref().unwrap()), "ping");
        assert_eq!(args.as_deref(), Some("hello"));
    }
}

//! Channel trait — abstract gateway for input/output.

pub mod telegram;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A single inbound message from a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Stable channel id (e.g. `telegram`).
    pub channel: String,
    /// Channel-specific user/chat id within the channel.
    pub chat_id: String,
    /// User id (optional).
    pub user_id: Option<String>,
    /// Display name (optional).
    pub user_name: Option<String>,
    /// When the message was sent.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Raw text body.
    pub text: String,
    /// Slash command (without the leading `/`), if present.
    pub command: Option<String>,
    /// Trailing arguments after the slash command.
    pub args: Option<String>,
}

/// Anything that can deliver inbound messages to the agent and
/// send replies back.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Stable identifier for this channel (e.g. `telegram`).
    fn name(&self) -> &str;

    /// Run the receive loop until cancelled.
    async fn run(&self, tx: tokio::sync::mpsc::Sender<InboundMessage>) -> Result<()>;

    /// Send a reply back to the channel.
    async fn send(&self, chat_id: &str, text: &str) -> Result<()>;
}

/// Parse a plain text message into a `(command, args)` pair when the text
/// starts with `/`. Returns `(None, None)` for non-command messages.
pub fn parse_command(text: &str) -> (Option<String>, Option<String>) {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('/') {
        return (None, None);
    }
    let rest = &trimmed[1..];
    let mut parts = rest.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("").to_string();
    let args = parts
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if cmd.is_empty() {
        (None, None)
    } else {
        (Some(cmd), args)
    }
}

/// Detect a `@botname` suffix on a slash command and strip it out.
pub fn strip_bot_mention(cmd: &str) -> String {
    cmd.split('@').next().unwrap_or(cmd).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_command() {
        let (cmd, args) = parse_command("/hello");
        assert_eq!(cmd.as_deref(), Some("hello"));
        assert_eq!(args, None);
    }

    #[test]
    fn parse_command_with_args() {
        let (cmd, args) = parse_command("/echo hi there");
        assert_eq!(cmd.as_deref(), Some("echo"));
        assert_eq!(args.as_deref(), Some("hi there"));
    }

    #[test]
    fn parse_command_with_mention() {
        let (cmd, _) = parse_command("/start@mybot");
        assert_eq!(cmd.as_deref(), Some("start@mybot"));
        assert_eq!(strip_bot_mention(cmd.as_deref().unwrap()), "start");
    }

    #[test]
    fn parse_no_command() {
        let (cmd, args) = parse_command("hello world");
        assert!(cmd.is_none());
        assert!(args.is_none());
    }
}

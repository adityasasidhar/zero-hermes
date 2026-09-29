//! Tool trait and Anthropic-flavoured tool call parsing.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

/// Context handed to a tool at execution time.
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// Working directory (default: current dir).
    pub cwd: Option<std::path::PathBuf>,
    /// Per-session conversation id (used by memory tool).
    pub session_id: Option<String>,
    /// Memory store handle for tools that need it (e.g. `MemoryTool`).
    pub memory: Option<std::sync::Arc<crate::memory::Memory>>,
}

/// Output of a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Primary textual result (always populated).
    pub content: String,
    /// Whether the tool execution failed.
    pub is_error: bool,
}

impl ToolOutput {
    /// Build a successful output.
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    /// Build a failed output.
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// A callable tool exposed to the agent.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Stable identifier used in tool calls and JSON schema.
    fn name(&self) -> &str;

    /// Human-readable description used in the schema.
    fn description(&self) -> &str;

    /// JSON Schema (`input_schema`) for the tool's arguments.
    fn schema(&self) -> Value;

    /// Run the tool with the given input.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;

    /// Whether the tool is free of observable side effects and safe to run
    /// concurrently with other read-only tools.
    ///
    /// Defaults to `false` (serial execution) so a new tool is safe until
    /// its implementation explicitly opts into parallelism. Read-only
    /// tools (`read`, `fetch`) override this to `true`.
    fn is_read_only(&self) -> bool {
        false
    }
}

/// Anthropic-flavoured `tool_use` block parsed from an LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique id assigned by the provider (used to match `tool_result`).
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Parsed input object.
    pub input: Value,
}

/// Anthropic-flavoured `tool_result` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Matching id from the `tool_use` block.
    pub tool_use_id: String,
    /// Result content.
    pub content: String,
    /// Whether the tool reported an error.
    pub is_error: bool,
}

impl ToolResult {
    /// Build a successful tool result.
    pub fn ok(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// Build a failed tool result.
    pub fn err(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
        }
    }
}

/// Build a `ContentBlock` that represents a tool result. Useful when the
/// caller wants to insert a `tool_result` block into an existing message.
pub fn tool_result_block(
    tool_use_id: impl Into<String>,
    content: impl Into<String>,
    is_error: bool,
) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_use_id.into(),
        content: content.into(),
        is_error,
    }
}

/// A single content block within an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text.
    Text { text: String },
    /// Tool invocation request.
    ToolUse(ToolCall),
    /// Tool result block (only used in `user` messages carrying results).
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// A single message in the agent conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: `user`, `assistant`, or `system`.
    pub role: String,
    /// Content blocks. For `user`/`system` this is usually a single `text`
    /// block; for `assistant` it may contain `tool_use` blocks.
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Build a user message with one text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Build a system message with one text block.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Build an assistant message that is just text.
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Build an assistant message from arbitrary content blocks.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }

    /// Build a user message carrying tool results.
    pub fn tool_results(results: Vec<ToolResult>) -> Self {
        let blocks = results
            .into_iter()
            .map(|r| ContentBlock::ToolResult {
                tool_use_id: r.tool_use_id,
                content: r.content,
                is_error: r.is_error,
            })
            .collect();
        Self {
            role: "user".to_string(),
            content: blocks,
        }
    }

    /// Concatenate all text in the message (empty string if none).
    pub fn text(&self) -> String {
        let mut out = String::new();
        for block in &self.content {
            match block {
                ContentBlock::Text { text } => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(text);
                }
                ContentBlock::ToolResult { content, .. } => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(content);
                }
                ContentBlock::ToolUse(_) => {}
            }
        }
        out
    }
}

/// Drop tool blocks that lost their partner across turns.
///
/// Both wire formats reject unpaired tool traffic outright: Anthropic 400s
/// on a `tool_result` with no preceding `tool_use` (and vice versa), and
/// OpenAI-compat endpoints reject `role:"tool"` messages with no matching
/// assistant `tool_calls` — strict validators also reject unanswered
/// `tool_calls`. History loaded from SQLite can contain orphans (an older
/// turn persisted results without their assistant message), and without
/// this every future turn re-sends the poison and fails the same way, which
/// bricks the session permanently: the failed turn is repaired away, but
/// the poisoned prefix is re-sent untouched.
///
/// Matching is positional and index-precise: a result answers a use only
/// from the immediately following user message, which is the shape `run`
/// always produces (a result id that was answered in some *other* pair
/// does not rescue an orphan elsewhere — call ids are unique per call).
/// The stored transcript is never touched — only the outgoing request is
/// cleaned. A message left with no blocks at all is dropped.
pub fn sanitize_tool_pairing(history: &[Message]) -> Vec<Message> {
    /// `tool_use` ids introduced by the assistant message at `idx`.
    fn uses_at(history: &[Message], idx: usize) -> Vec<&str> {
        if history[idx].role != "assistant" {
            return Vec::new();
        }
        history[idx]
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse(tc) => Some(tc.id.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Is the `tool_use` id of the assistant message at `idx` answered by
    /// a `tool_result` in the immediately following user message?
    fn use_answered(history: &[Message], idx: usize, id: &str) -> bool {
        let Some(next) = history.get(idx + 1) else {
            return false;
        };
        if next.role != "user" {
            return false;
        }
        next.content.iter().any(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => tool_use_id == id,
            _ => false,
        })
    }

    let mut out = Vec::with_capacity(history.len());
    for (i, m) in history.iter().enumerate() {
        match m.role.as_str() {
            "assistant" => {
                let mut content = Vec::with_capacity(m.content.len());
                let mut dropped: Vec<String> = Vec::new();
                for b in &m.content {
                    match b {
                        ContentBlock::Text { .. } => content.push(b.clone()),
                        ContentBlock::ToolUse(tc) if use_answered(history, i, &tc.id) => {
                            content.push(b.clone());
                        }
                        ContentBlock::ToolUse(tc) => dropped.push(tc.name.clone()),
                        ContentBlock::ToolResult { .. } => {
                            // A result parked in an assistant message is
                            // never valid; drop it rather than emit it.
                        }
                    }
                }
                if content.is_empty() && !dropped.is_empty() {
                    // Keep a labelled placeholder so role alternation
                    // survives the repair: dropping the message outright
                    // could glue two user turns together, which Anthropic
                    // also rejects.
                    content.push(ContentBlock::Text {
                        text: format!(
                            "[dropped {} orphaned tool call(s) without a recorded result: {}]",
                            dropped.len(),
                            dropped.join(", ")
                        ),
                    });
                }
                if !content.is_empty() {
                    out.push(Message {
                        role: m.role.clone(),
                        content,
                    });
                }
            }
            "user" => {
                // A result is kept only when the immediately preceding
                // message is the assistant turn that introduced its id.
                let uses = if i > 0 {
                    uses_at(history, i - 1)
                } else {
                    Vec::new()
                };
                let mut content = Vec::with_capacity(m.content.len());
                for b in &m.content {
                    match b {
                        ContentBlock::ToolResult { tool_use_id, .. }
                            if !uses.contains(&tool_use_id.as_str()) =>
                        {
                            // Orphan result: sending it would 400 the
                            // request on either wire format.
                        }
                        _ => content.push(b.clone()),
                    }
                }
                if !content.is_empty() {
                    out.push(Message {
                        role: m.role.clone(),
                        content,
                    });
                }
            }
            // System and anything else pass through untouched.
            _ => out.push(m.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trip() {
        let m = Message::user("hello");
        assert_eq!(m.role, "user");
        assert_eq!(m.text(), "hello");
    }

    #[test]
    fn tool_result_block_serialises() {
        let m = Message::tool_results(vec![ToolResult::ok("id1", "ok")]);
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["content"][0]["type"], "tool_result");
        assert_eq!(v["content"][0]["tool_use_id"], "id1");
    }

    #[test]
    fn tool_result_block_content_extractable() {
        let m = Message::tool_results(vec![ToolResult::ok("id1", "hi")]);
        let mut saw = false;
        for b in &m.content {
            if let ContentBlock::ToolResult { content, .. } = b {
                assert_eq!(content, "hi");
                saw = true;
            }
        }
        assert!(saw);
    }

    fn paired_turn(id: &str) -> Vec<Message> {
        vec![
            Message::assistant(vec![ContentBlock::ToolUse(ToolCall {
                id: id.into(),
                name: "bash".into(),
                input: serde_json::json!({}),
            })]),
            Message::tool_results(vec![ToolResult::ok(id, "out")]),
        ]
    }

    #[test]
    fn sanitize_keeps_paired_traffic_untouched() {
        let history = vec![
            Message::user("hi"),
            Message::assistant_text("ok"),
            paired_turn("c1")[0].clone(),
            paired_turn("c1")[1].clone(),
            Message::user("thanks"),
        ];
        let clean = sanitize_tool_pairing(&history);
        assert_eq!(clean.len(), history.len(), "paired history must survive");
    }

    #[test]
    fn sanitize_drops_an_orphan_tool_result() {
        // The exact poison from a real session: a user message carrying
        // only a `tool_result` whose assistant `tool_use` is gone. Sent
        // as-is, strict OpenAI-compat validators 400 the whole request.
        let history = vec![
            Message::user("hey"),
            Message::assistant_text("Hey!"),
            Message::tool_results(vec![ToolResult::ok("ghost-1", "stale output")]),
            Message::assistant_text("done"),
            Message::user("hey again"),
        ];
        let clean = sanitize_tool_pairing(&history);
        assert_eq!(clean.len(), 4, "the orphan message must go");
        assert!(
            !clean.iter().any(|m| m.text().contains("stale output")),
            "orphan content must not leak into the request"
        );
        assert_eq!(clean.last().unwrap().text(), "hey again");
    }

    #[test]
    fn sanitize_keeps_text_beside_an_orphan_result() {
        let mut mixed = Message::user("note this");
        mixed.content.push(ContentBlock::ToolResult {
            tool_use_id: "ghost-2".into(),
            content: "stale".into(),
            is_error: false,
        });
        let clean = sanitize_tool_pairing(&[mixed]);
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0].text(), "note this");
    }

    #[test]
    fn sanitize_strips_unanswered_tool_use_but_keeps_alternation() {
        let history = vec![
            Message::user("a"),
            Message::assistant(vec![ContentBlock::ToolUse(ToolCall {
                id: "lost-1".into(),
                name: "bash".into(),
                input: serde_json::json!({}),
            })]),
            Message::user("b"),
        ];
        let clean = sanitize_tool_pairing(&history);
        assert_eq!(clean.len(), 3, "placeholder keeps user/assistant/user");
        assert_eq!(clean[0].role, "user");
        assert_eq!(clean[1].role, "assistant");
        assert_eq!(clean[2].role, "user");
        assert!(
            clean[1].text().contains("orphaned tool call"),
            "placeholder must be labelled: {}",
            clean[1].text()
        );
    }

    #[test]
    fn sanitize_matching_is_positional_not_global() {
        // id "c1" is properly answered in the first pair; a later message
        // reusing the bare id shape with no adjacent use is still an orphan.
        // (Call ids are unique per call, so this is defence in depth.)
        let history = vec![
            paired_turn("c1")[0].clone(),
            paired_turn("c1")[1].clone(),
            Message::tool_results(vec![ToolResult::ok("c1", "replay")]),
            Message::user("next"),
        ];
        let clean = sanitize_tool_pairing(&history);
        assert!(
            !clean.iter().any(|m| m.text().contains("replay")),
            "a result answered elsewhere must not rescue this orphan"
        );
    }
}

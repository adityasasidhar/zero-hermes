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
}

//! LLM provider abstraction and the Anthropic-Messages implementation
//! (which is the wire format the `minimax` endpoint at
//! `https://api.minimax.io/anthropic` speaks).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool::{ContentBlock, Message, ToolCall};
use crate::config::ProviderConfig;
use crate::error::Result;

/// What an LLM produced for a single `complete` call.
#[derive(Debug, Clone)]
pub struct Completion {
    /// Final text content (None if the model only emitted tool calls).
    pub text: Option<String>,
    /// Tool calls requested by the model (empty if it produced text).
    pub tool_calls: Vec<ToolCall>,
}

impl Completion {
    /// Did the model hand back a pure-text response?
    pub fn is_final(&self) -> bool {
        self.tool_calls.is_empty() && self.text.is_some()
    }
}

/// Pluggable LLM provider.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send the conversation plus available tool schemas and return a
    /// completion. The provider may issue at most one round-trip per call;
    /// the outer agent loop handles iteration.
    async fn complete(
        &self,
        system: Option<&str>,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<Completion>;
}

/// Anthropic-Messages provider (compatible with the minimax endpoint).
pub struct AnthropicMessages {
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    client: reqwest::Client,
}

impl AnthropicMessages {
    /// Build a new provider from a [`ProviderConfig`].
    pub fn new(cfg: &ProviderConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| anyhow::anyhow!("reqwest client build: {e}"))?;
        Ok(Self {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            max_tokens: cfg.max_tokens,
            client,
        })
    }

    /// Build the request body for /v1/messages.
    pub fn build_body(system: Option<&str>, messages: &[Message], tools: &[Value]) -> Value {
        let rendered: Vec<Value> = messages
            .iter()
            .map(|m| {
                let content: Vec<Value> = m
                    .content
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text { text } => json!({"type": "text", "text": text}),
                        ContentBlock::ToolUse(tc) => json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.input,
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                            "is_error": is_error,
                        }),
                    })
                    .collect();
                json!({"role": m.role, "content": content})
            })
            .collect();

        let mut body = json!({
            "model": "placeholder",
            "max_tokens": 0u32,
            "messages": rendered,
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        body
    }

    /// Parse a /v1/messages response.
    pub fn parse_response(body: &Value) -> Result<Completion> {
        let content = body
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("response missing content array"))?;

        let mut text: Option<String> = None;
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for block in content {
            match block.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    let t = block
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !t.is_empty() {
                        text = Some(match text {
                            Some(prev) => format!("{prev}\n{t}"),
                            None => t,
                        });
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("tool_use without id"))?
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("tool_use without name"))?
                        .to_string();
                    let input = block.get("input").cloned().unwrap_or(Value::Null);
                    tool_calls.push(ToolCall { id, name, input });
                }
                _ => {}
            }
        }
        Ok(Completion { text, tool_calls })
    }

    /// Provider name (used by mock providers and registry).
    pub fn model_name(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProvider for AnthropicMessages {
    async fn complete(
        &self,
        system: Option<&str>,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<Completion> {
        let mut body = Self::build_body(system, messages, tools);
        body["model"] = json!(self.model);
        body["max_tokens"] = json!(self.max_tokens);

        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LLM request failed: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("LLM body read: {e}"))?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "LLM returned {}: {}",
                status,
                truncate(&text, 256)
            ));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("LLM body not JSON: {e}: {}", truncate(&text, 256)))?;
        Self::parse_response(&parsed)
    }
}

/// Truncate a string for error messages.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut t = s[..max].to_string();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool::Message;

    #[test]
    fn build_body_minimal() {
        let body = AnthropicMessages::build_body(Some("sys"), &[Message::user("hi")], &[]);
        assert_eq!(body["system"], "sys");
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn parse_text_response() {
        let v = json!({"content": [{"type": "text", "text": "hi"}]});
        let c = AnthropicMessages::parse_response(&v).unwrap();
        assert_eq!(c.text.as_deref(), Some("hi"));
        assert!(c.tool_calls.is_empty());
    }

    #[test]
    fn parse_tool_use_response() {
        let v = json!({"content": [
            {"type": "tool_use", "id": "x1", "name": "bash", "input": {"command": "ls"}}
        ]});
        let c = AnthropicMessages::parse_response(&v).unwrap();
        assert_eq!(c.tool_calls.len(), 1);
        assert_eq!(c.tool_calls[0].name, "bash");
    }
}

//! LLM provider abstraction. Two implementations ship:
//!  - [`AnthropicMessages`] — the wire format the `minimax` endpoint at
//!    `https://api.minimax.io/anthropic` speaks.
//!  - [`OpenAiCompat`] — any OpenAI-compatible `/v1/chat/completions` endpoint
//!    (Together, Groq, OpenRouter, llama.cpp, ollama, etc.).
//!
//! Both implement [`LlmProvider`]; the agent loop is provider-agnostic.

use async_trait::async_trait;
use futures::Stream;
use futures::StreamExt;
use serde_json::{json, Value};
use std::pin::Pin;

use crate::agent::stream::{one_shot_stream, EventStream, StreamEvent};
use crate::agent::tool::{ContentBlock, Message, ToolCall};
use crate::config::{ProviderConfig, ProviderKind};
use crate::error::Result;
use crate::util::truncate_bytes;

/// Build the shared HTTP client for a provider.
///
/// `read_timeout` is per-read rather than a whole-request deadline, so a
/// slow-but-alive streaming completion is fine while a provider that stops
/// responding mid-response is not. Without any timeout a wedged provider
/// hangs the gateway indefinitely, and because the gateway dispatches
/// turns serially that stalls every chat and every cron tick with it.
fn build_client(cfg: &ProviderConfig) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(cfg.connect_timeout_secs))
        .read_timeout(std::time::Duration::from_secs(cfg.read_timeout_secs))
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest client build: {e}"))
}

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

    /// Streaming variant. Returns a stream of [`StreamEvent`]s ending in a
    /// single `Done(Completion)`. Default implementation just calls
    /// `complete()` and emits a one-event stream — providers that support
    /// real Server-Sent-Events should override this.
    fn stream<'a>(
        &'a self,
        system: Option<&'a str>,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + 'a>>
    where
        Self: 'a,
    {
        one_shot_stream(self, system, messages, tools)
    }
}

// Convenience: shorthand for the boxed event stream.
#[allow(dead_code)]
type EventStreamAlias = EventStream;

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
        Ok(Self {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            max_tokens: cfg.max_tokens,
            client: build_client(cfg)?,
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

        let mut text_buf = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut first_text = true;

        for block in content {
            match block.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    let t = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    if !t.is_empty() {
                        if !first_text {
                            text_buf.push('\n');
                        }
                        first_text = false;
                        text_buf.push_str(t);
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
        let text = if text_buf.is_empty() {
            None
        } else {
            Some(text_buf)
        };
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
                truncate_bytes(&text, 256)
            ));
        }
        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("LLM body not JSON: {e}: {}", truncate_bytes(&text, 256))
        })?;
        Self::parse_response(&parsed)
    }

    fn stream<'a>(
        &'a self,
        system: Option<&'a str>,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = crate::error::Result<StreamEvent>> + Send + 'a>>
    where
        Self: 'a,
    {
        // Issue a `stream: true` request synchronously, then return the SSE
        // parser over the resulting response. Any HTTP error here surfaces
        // as the first stream event.
        let mut body = Self::build_body(system, messages, tools);
        body["model"] = json!(self.model);
        body["max_tokens"] = json!(self.max_tokens);
        body["stream"] = json!(true);

        let url = format!("{}/v1/messages", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        Box::pin(
            futures::stream::once(async move {
                let resp = client
                    .post(&url)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("LLM streaming request failed: {e}"))?;

                let status = resp.status();
                if !status.is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!(
                        "LLM returned {}: {}",
                        status,
                        truncate_bytes(&text, 256)
                    ));
                }
                Ok(resp)
            })
            .flat_map(|r: crate::error::Result<reqwest::Response>| match r {
                Ok(resp) => crate::agent::stream::parse_anthropic_sse(resp),
                Err(e) => Box::pin(futures::stream::once(async move { Err(e) }))
                    as std::pin::Pin<
                        Box<dyn futures::Stream<Item = crate::error::Result<StreamEvent>> + Send>,
                    >,
            }),
        )
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

/// OpenAI-compatible `/v1/chat/completions` provider.
///
/// Talks to any server implementing the OpenAI Chat Completions API:
/// OpenAI, Together, Groq, OpenRouter, llama.cpp's `server`, ollama's
/// `/v1/chat/completions` shim, etc.
///
/// Tools are surfaced in OpenAI's `[{type:"function", function:{...}}]` form,
/// assistant tool calls come back as `tool_calls: [{id, type, function:{name, arguments}}]`,
/// and tool results are returned as separate `{role:"tool", tool_call_id, content}` messages.
pub struct OpenAiCompat {
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: Option<f32>,
    client: reqwest::Client,
}

/// Concatenate text blocks in a message into a single newline-separated
/// string. Returns an empty string if there are no text blocks.
fn join_text_blocks(content: &[ContentBlock]) -> String {
    let mut out = String::new();
    let mut first = true;
    for b in content {
        if let ContentBlock::Text { text } = b {
            if !first {
                out.push('\n');
            }
            first = false;
            out.push_str(text);
        }
    }
    out
}

/// Does this content list carry any `tool_result` blocks?
fn has_tool_results(content: &[ContentBlock]) -> bool {
    content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

/// Append one `role:"tool"` message per `tool_result` block in `content`.
fn push_tool_results(msgs: &mut Vec<Value>, content: &[ContentBlock]) {
    for b in content {
        let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = b
        else {
            continue;
        };
        // OpenAI has no `is_error` flag; embed it in the content so
        // downstream models (and humans reading logs) can see it.
        let body = if *is_error {
            format!("[is_error=true] {content}")
        } else {
            content.clone()
        };
        msgs.push(json!({
            "role": "tool",
            "tool_call_id": tool_use_id,
            "content": body,
        }));
    }
}

impl OpenAiCompat {
    /// Build a new provider from a [`ProviderConfig`].
    pub fn new(cfg: &ProviderConfig) -> Result<Self> {
        Ok(Self {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            max_tokens: cfg.max_tokens,
            temperature: cfg.temperature,
            client: build_client(cfg)?,
        })
    }

    /// Provider name (the configured model id).
    pub fn model_name(&self) -> &str {
        &self.model
    }

    /// Convert internal `Message`s + tool schemas to the OpenAI request shape.
    pub fn build_body(system: Option<&str>, messages: &[Message], tools: &[Value]) -> Value {
        let mut msgs: Vec<Value> = Vec::new();

        if let Some(sys) = system {
            msgs.push(json!({"role": "system", "content": sys}));
        }

        for m in messages {
            match m.role.as_str() {
                // User message. The agent loop parks tool results in a
                // *user*-role message (that is the Anthropic shape, see
                // `Message::tool_results`), so a user turn may carry
                // `tool_result` blocks, text, or both. OpenAI wants the
                // results as separate `role:"tool"` messages, and they
                // must come first so each one directly answers the
                // preceding assistant `tool_calls`.
                "user" => {
                    push_tool_results(&mut msgs, &m.content);
                    let text = join_text_blocks(&m.content);
                    if !text.is_empty() || !has_tool_results(&m.content) {
                        msgs.push(json!({"role": "user", "content": text}));
                    }
                }

                // Assistant message: collapse text, then emit any tool_calls.
                "assistant" => {
                    let text = join_text_blocks(&m.content);
                    let tool_calls: Vec<Value> = m
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::ToolUse(tc) => Some(json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    // OpenAI wants a *string* of JSON for arguments;
                                    // some servers parse it, others want an object — we send a string.
                                    "arguments": serde_json::to_string(&tc.input)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                }
                            })),
                            _ => None,
                        })
                        .collect();
                    let mut msg = json!({"role": "assistant"});
                    // Some OpenAI-compatible servers (llama.cpp server, vLLM with
                    // strict validators) reject `content: null`. Use empty
                    // string for pure tool-call messages; null only when there
                    // are no tool calls and the message is purely text-less
                    // (which the spec allows but most servers tolerate).
                    if !text.is_empty() {
                        msg["content"] = json!(text);
                    } else if tool_calls.is_empty() {
                        msg["content"] = json!("");
                    } else {
                        msg["content"] = Value::Null;
                    }
                    if !tool_calls.is_empty() {
                        msg["tool_calls"] = json!(tool_calls);
                    }
                    msgs.push(msg);
                }

                // An explicit "tool" role, should a caller construct one
                // directly, maps 1:1.
                "tool" => push_tool_results(&mut msgs, &m.content),

                // Anything we don't recognise: send as plain text user message
                // (best-effort fallback so we never drop a turn).
                other => {
                    let text = join_text_blocks(&m.content);
                    msgs.push(json!({"role": other, "content": text}));
                }
            }
        }

        let mut body = json!({
            "model": "placeholder",
            "max_tokens": 0u32,
            "messages": msgs,
        });
        if !tools.is_empty() {
            // The internal tool schemas are already JSON Schema objects;
            // wrap them in the OpenAI `{type:"function", function:{...}}` envelope.
            let wrapped: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                            "description": t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "parameters": t.get("input_schema").cloned().unwrap_or_else(|| json!({"type": "object"})),
                        }
                    })
                })
                .collect();
            body["tools"] = json!(wrapped);
        }
        body
    }

    /// Parse a `/v1/chat/completions` response into our internal Completion.
    pub fn parse_response(body: &Value) -> Result<Completion> {
        let choice = body
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow::anyhow!("response missing choices[0]"))?;
        let msg = choice
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("choice missing message"))?;

        let mut text: Option<String> = None;
        if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                text = Some(content.to_string());
            }
        }

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        if let Some(arr) = msg.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in arr {
                let id = tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("tool_call missing id"))?
                    .to_string();
                let func = tc
                    .get("function")
                    .ok_or_else(|| anyhow::anyhow!("tool_call missing function object"))?;
                let name = func
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("tool_call.function missing name"))?
                    .to_string();
                // `arguments` may arrive as a JSON string (OpenAI spec) or already-parsed object.
                let input = match func.get("arguments") {
                    Some(Value::String(s)) => {
                        serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
                    }
                    Some(v) => v.clone(),
                    None => Value::Null,
                };
                tool_calls.push(ToolCall { id, name, input });
            }
        }

        Ok(Completion { text, tool_calls })
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompat {
    async fn complete(
        &self,
        system: Option<&str>,
        messages: &[Message],
        tools: &[Value],
    ) -> Result<Completion> {
        let mut body = Self::build_body(system, messages, tools);
        body["model"] = json!(self.model);
        body["max_tokens"] = json!(self.max_tokens);
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }

        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
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
                truncate_bytes(&text, 256)
            ));
        }
        let parsed: Value = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("LLM body not JSON: {e}: {}", truncate_bytes(&text, 256))
        })?;
        Self::parse_response(&parsed)
    }

    fn stream<'a>(
        &'a self,
        system: Option<&'a str>,
        messages: &'a [Message],
        tools: &'a [Value],
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = crate::error::Result<StreamEvent>> + Send + 'a>>
    where
        Self: 'a,
    {
        let mut body = Self::build_body(system, messages, tools);
        body["model"] = json!(self.model);
        body["max_tokens"] = json!(self.max_tokens);
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        body["stream"] = json!(true);

        let url = format!("{}/v1/chat/completions", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        Box::pin(
            futures::stream::once(async move {
                let resp = client
                    .post(&url)
                    .bearer_auth(&api_key)
                    .header("content-type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("LLM streaming request failed: {e}"))?;

                let status = resp.status();
                if !status.is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!(
                        "LLM returned {}: {}",
                        status,
                        truncate_bytes(&text, 256)
                    ));
                }
                Ok(resp)
            })
            .flat_map(|r: crate::error::Result<reqwest::Response>| match r {
                Ok(resp) => crate::agent::stream::parse_openai_sse(resp),
                Err(e) => Box::pin(futures::stream::once(async move { Err(e) }))
                    as std::pin::Pin<
                        Box<dyn futures::Stream<Item = crate::error::Result<StreamEvent>> + Send>,
                    >,
            }),
        )
    }
}

/// Factory: build the right provider from a [`ProviderConfig`].
pub fn build_provider(cfg: &ProviderConfig) -> Result<Box<dyn LlmProvider>> {
    match cfg.kind {
        ProviderKind::Anthropic => Ok(Box::new(AnthropicMessages::new(cfg)?)),
        ProviderKind::OpenaiCompat => Ok(Box::new(OpenAiCompat::new(cfg)?)),
    }
}

#[cfg(test)]
mod openai_tests {
    use super::*;
    use crate::agent::tool::{ContentBlock, Message, ToolCall};

    fn sample_tool_schema() -> Value {
        json!({
            "name": "bash",
            "description": "Run a shell command",
            "input_schema": {
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"],
            }
        })
    }

    #[test]
    fn openai_body_minimal() {
        let body = OpenAiCompat::build_body(Some("sys"), &[Message::user("hi")], &[]);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "sys");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "hi");
    }

    #[test]
    fn openai_body_wraps_tools() {
        let body = OpenAiCompat::build_body(None, &[Message::user("hi")], &[sample_tool_schema()]);
        let t = &body["tools"][0];
        assert_eq!(t["type"], "function");
        assert_eq!(t["function"]["name"], "bash");
        assert_eq!(
            t["function"]["parameters"]["properties"]["command"]["type"],
            "string"
        );
    }

    #[test]
    fn openai_body_emits_tool_role_messages() {
        // Build the history exactly the way `agent::run` does. It parks
        // tool results in a *user*-role message, and an earlier version of
        // this test hand-wrote `role: "tool"` instead — a shape production
        // never emits — which is why the results being silently dropped
        // went unnoticed.
        let assistant = Message::assistant(vec![ContentBlock::ToolUse(ToolCall {
            id: "call_1".into(),
            name: "bash".into(),
            input: json!({"command": "ls"}),
        })]);
        let tool_result = Message::tool_results(vec![crate::agent::tool::ToolResult::ok(
            "call_1",
            "main.rs\n",
        )]);
        assert_eq!(tool_result.role, "user", "this is the shape the loop uses");

        let body = OpenAiCompat::build_body(None, &[assistant, tool_result], &[]);
        let msgs = body["messages"].as_array().expect("messages array");

        // assistant message has tool_calls array
        assert_eq!(msgs[0]["role"], "assistant");
        assert_eq!(msgs[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(msgs[0]["tool_calls"][0]["function"]["name"], "bash");
        // arguments is a JSON *string* (per OpenAI spec)
        let args_str = msgs[0]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .expect("arguments should be string");
        assert!(args_str.contains("\"command\""));

        // ...and the result answers it as its own role:tool message.
        assert_eq!(msgs.len(), 2, "no empty filler user turn: {msgs:?}");
        assert_eq!(msgs[1]["role"], "tool");
        assert_eq!(msgs[1]["tool_call_id"], "call_1");
        assert_eq!(msgs[1]["content"], "main.rs\n");
    }

    #[test]
    fn openai_body_keeps_plain_user_turns() {
        let body = OpenAiCompat::build_body(None, &[Message::user("hi")], &[]);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "hi");
    }

    #[test]
    fn openai_body_handles_multiple_results_in_one_turn() {
        // A single assistant turn can request several tools; the loop packs
        // every result into one user message, and each needs its own
        // role:tool reply keyed by tool_call_id.
        let m = Message::tool_results(vec![
            crate::agent::tool::ToolResult::ok("call_a", "out-a"),
            crate::agent::tool::ToolResult::err("call_b", "boom"),
        ]);
        let body = OpenAiCompat::build_body(None, &[m], &[]);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["tool_call_id"], "call_a");
        assert_eq!(msgs[0]["content"], "out-a");
        assert_eq!(msgs[1]["tool_call_id"], "call_b");
        assert_eq!(msgs[1]["content"], "[is_error=true] boom");
    }

    #[test]
    fn openai_body_marks_is_error_in_content() {
        let tool_result = Message::tool_results(vec![crate::agent::tool::ToolResult::err(
            "call_1",
            "command timed out",
        )]);
        let body = OpenAiCompat::build_body(None, &[tool_result], &[]);
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert!(content.starts_with("[is_error=true]"));
    }

    #[test]
    fn openai_parse_text_response() {
        let v = json!({
            "choices": [{
                "message": {"role": "assistant", "content": "hello"}
            }]
        });
        let c = OpenAiCompat::parse_response(&v).unwrap();
        assert_eq!(c.text.as_deref(), Some("hello"));
        assert!(c.tool_calls.is_empty());
    }

    #[test]
    fn openai_parse_tool_call_with_string_arguments() {
        let v = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "bash",
                            "arguments": "{\"command\":\"ls\"}"
                        }
                    }]
                }
            }]
        });
        let c = OpenAiCompat::parse_response(&v).unwrap();
        assert_eq!(c.tool_calls.len(), 1);
        assert_eq!(c.tool_calls[0].id, "call_abc");
        assert_eq!(c.tool_calls[0].name, "bash");
        assert_eq!(c.tool_calls[0].input["command"], "ls");
    }

    #[test]
    fn openai_parse_tool_call_with_object_arguments() {
        // Some servers already parse arguments; tolerate that too.
        let v = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_xyz",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": {"path": "/etc/hostname"}
                        }
                    }]
                }
            }]
        });
        let c = OpenAiCompat::parse_response(&v).unwrap();
        assert_eq!(c.tool_calls[0].input["path"], "/etc/hostname");
    }

    #[test]
    fn openai_parse_missing_choices_errors() {
        let v = json!({"error": "bad request"});
        assert!(OpenAiCompat::parse_response(&v).is_err());
    }
}

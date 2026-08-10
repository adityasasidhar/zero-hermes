//! Streaming responses for LLM providers.
//!
//! Providers that support streaming expose a `stream()` method that returns
//! an async [`Stream`] of [`StreamEvent`]s. Providers that don't (or callers
//! that don't care) get a default implementation that runs `complete()` and
//! returns a single-event stream.
//!
//! The actual Server-Sent-Events parsers live in this file: a tiny hand-rolled
//! state machine that consumes the byte stream and yields events. This is
//! deliberately minimal — we only implement the parts of SSE that real
//! providers (Anthropic, OpenAI-compatible) actually emit.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use serde_json::Value;

use crate::agent::provider::{Completion, LlmProvider};
use crate::agent::tool::ToolCall;
use crate::error::Result;

type ByteResult = std::result::Result<bytes::Bytes, reqwest::Error>;
type ByteStream = Pin<Box<dyn Stream<Item = ByteResult> + Send + 'static>>;

/// What an LLM produced for a single `stream` call. Callers should treat
/// this as a sequence: zero or more `TextDelta`/`ToolUseBlock` events
/// followed by exactly one `Done` carrying the accumulated completion.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of generated text. May arrive in many small pieces.
    TextDelta(String),
    /// A tool invocation block has finished accumulating. The associated
    /// `ToolCall` is fully formed (id, name, input).
    ToolUseBlock(ToolCall),
    /// Stream finished. The `Completion` is the consolidated final answer
    /// (text + tool calls), equivalent to what `LlmProvider::complete()`
    /// would have returned.
    Done(Completion),
}

/// A boxed, type-erased, `Send` stream of [`StreamEvent`]s. This is what
/// every provider's `stream()` method returns.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;

// ---------------------------------------------------------------------------
// Default `stream()` implementation for any LlmProvider.
// ---------------------------------------------------------------------------

/// Wrap a single `complete()` call into a one-event stream. This is the
/// default for providers that don't have a real streaming implementation.
pub fn one_shot_stream<'a, P: LlmProvider + ?Sized>(
    provider: &'a P,
    system: Option<&'a str>,
    messages: &'a [crate::agent::tool::Message],
    tools: &'a [Value],
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + 'a>> {
    let fut = provider.complete(system, messages, tools);
    Box::pin(futures::stream::once(async move {
        fut.await.map(StreamEvent::Done)
    }))
}

// ---------------------------------------------------------------------------
// Anthropic SSE parser
// ---------------------------------------------------------------------------

/// Parse an Anthropic `/v1/messages` SSE response body into a stream of
/// [`StreamEvent`]s. The connection is fully consumed; the returned stream
/// yields exactly one `Done` event at the end.
pub fn parse_anthropic_sse(response: reqwest::Response) -> EventStream {
    Box::pin(AnthropicSse {
        inner: Box::pin(response.bytes_stream()),
        buffer: String::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        // 0 = no current block, 1 = text block, 2 = tool_use block
        in_block: 0,
        cur_tool_id: String::new(),
        cur_tool_name: String::new(),
        cur_tool_input: String::new(),
        finished: false,
    })
}

struct AnthropicSse {
    inner: ByteStream,
    buffer: String,
    text: String,
    tool_calls: Vec<ToolCall>,
    in_block: u8,
    cur_tool_id: String,
    cur_tool_name: String,
    cur_tool_input: String,
    finished: bool,
}

impl AnthropicSse {
    fn feed_line(&mut self, line: &str) -> Result<Option<StreamEvent>> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(None);
        };
        let payload = data.trim();
        if payload.is_empty() {
            return Ok(None);
        }
        let v: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            // Some providers ping heartbeat JSON we don't care about; skip
            // anything that doesn't parse rather than failing the stream.
            Err(_) => return Ok(None),
        };
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "content_block_start" => {
                let block = v.get("content_block").cloned().unwrap_or(Value::Null);
                match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "text" => {
                        self.in_block = 1;
                    }
                    "tool_use" => {
                        self.in_block = 2;
                        self.cur_tool_id = block
                            .get("id")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        self.cur_tool_name = block
                            .get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        self.cur_tool_input.clear();
                    }
                    _ => {
                        self.in_block = 0;
                    }
                }
                Ok(None)
            }
            "content_block_delta" => {
                let delta = v.get("delta").cloned().unwrap_or(Value::Null);
                let dtype = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match dtype {
                    "text_delta" => {
                        if let Some(t) = delta.get("text").and_then(|x| x.as_str()) {
                            self.text.push_str(t);
                            return Ok(Some(StreamEvent::TextDelta(t.to_string())));
                        }
                    }
                    "input_json_delta" => {
                        if let Some(partial) = delta.get("partial_json").and_then(|x| x.as_str()) {
                            self.cur_tool_input.push_str(partial);
                        }
                    }
                    _ => {}
                }
                Ok(None)
            }
            "content_block_stop" => {
                if self.in_block == 2 {
                    let input: Value = serde_json::from_str(&self.cur_tool_input)
                        .unwrap_or_else(|_| Value::String(self.cur_tool_input.clone()));
                    let tc = ToolCall {
                        id: std::mem::take(&mut self.cur_tool_id),
                        name: std::mem::take(&mut self.cur_tool_name),
                        input,
                    };
                    self.tool_calls.push(tc.clone());
                    self.in_block = 0;
                    return Ok(Some(StreamEvent::ToolUseBlock(tc)));
                }
                self.in_block = 0;
                Ok(None)
            }
            "message_stop" => {
                self.finished = true;
                Ok(Some(StreamEvent::Done(Completion {
                    text: if self.text.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut self.text))
                    },
                    tool_calls: std::mem::take(&mut self.tool_calls),
                })))
            }
            // message_start / message_delta / ping etc — ignore.
            _ => Ok(None),
        }
    }

    fn flush_done_if_needed(&mut self) -> Option<StreamEvent> {
        if self.finished {
            return None;
        }
        // Stream ended without an explicit message_stop (rare but possible
        // for cancelled or truncated responses); still emit a Done.
        self.finished = true;
        Some(StreamEvent::Done(Completion {
            text: if self.text.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.text))
            },
            tool_calls: std::mem::take(&mut self.tool_calls),
        }))
    }
}

impl Stream for AnthropicSse {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to drain any complete lines already in the buffer.
            if let Some(idx) = self.buffer.find('\n') {
                let line = self.buffer[..idx].to_string();
                self.buffer.drain(..=idx);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                match self.feed_line(line) {
                    Ok(Some(ev)) => return Poll::Ready(Some(Ok(ev))),
                    Ok(None) => continue,
                    Err(e) => return Poll::Ready(Some(Err(e))),
                }
            }

            // Need more bytes from the underlying byte stream.
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                    continue;
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(anyhow::anyhow!("SSE read error: {e}"))));
                }
                Poll::Ready(None) => {
                    if let Some(ev) = self.flush_done_if_needed() {
                        return Poll::Ready(Some(Ok(ev)));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compat SSE parser
// ---------------------------------------------------------------------------

/// Parse an OpenAI-compatible `/v1/chat/completions` SSE response into a
/// stream of [`StreamEvent`]s. The end marker is `data: [DONE]`.
pub fn parse_openai_sse(response: reqwest::Response) -> EventStream {
    Box::pin(OpenAiSse {
        inner: Box::pin(response.bytes_stream()),
        buffer: String::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        // Per-index tool call accumulators keyed by `index` field.
        // We don't always receive the full tool call on first delta; the
        // first delta usually has the id+name and subsequent ones have
        // argument fragments.
        builders: std::collections::HashMap::new(),
        finished: false,
    })
}

struct OpenAiToolBuilder {
    id: String,
    name: String,
    args: String,
}

struct OpenAiSse {
    inner: ByteStream,
    buffer: String,
    text: String,
    tool_calls: Vec<ToolCall>,
    builders: std::collections::HashMap<usize, OpenAiToolBuilder>,
    finished: bool,
}

impl OpenAiSse {
    fn feed_line(&mut self, line: &str) -> Result<Option<StreamEvent>> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(None);
        };
        let payload = data.trim();
        if payload.is_empty() {
            return Ok(None);
        }
        if payload == "[DONE]" {
            self.finished = true;
            // Flush any in-flight tool calls.
            let mut out: Vec<ToolCall> = std::mem::take(&mut self.tool_calls);
            for (_, b) in self.builders.drain() {
                let input: Value =
                    serde_json::from_str(&b.args).unwrap_or_else(|_| Value::String(b.args.clone()));
                out.push(ToolCall {
                    id: b.id,
                    name: b.name,
                    input,
                });
            }
            return Ok(Some(StreamEvent::Done(Completion {
                text: if self.text.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.text))
                },
                tool_calls: out,
            })));
        }
        let v: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let choice = v
            .get("choices")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
        let mut emitted: Option<StreamEvent> = None;
        if let Some(content) = delta.get("content").and_then(|x| x.as_str()) {
            if !content.is_empty() {
                self.text.push_str(content);
                emitted = Some(StreamEvent::TextDelta(content.to_string()));
            }
        }
        if let Some(arr) = delta.get("tool_calls").and_then(|x| x.as_array()) {
            for tc in arr {
                let idx = tc.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let entry = self
                    .builders
                    .entry(idx)
                    .or_insert_with(|| OpenAiToolBuilder {
                        id: String::new(),
                        name: String::new(),
                        args: String::new(),
                    });
                if let Some(id) = tc.get("id").and_then(|x| x.as_str()) {
                    if !id.is_empty() {
                        entry.id = id.to_string();
                    }
                }
                if let Some(func) = tc.get("function") {
                    if let Some(name) = func.get("name").and_then(|x| x.as_str()) {
                        if !name.is_empty() {
                            entry.name = name.to_string();
                        }
                    }
                    if let Some(args) = func.get("arguments").and_then(|x| x.as_str()) {
                        entry.args.push_str(args);
                    }
                }
            }
        }
        // Tool calls sometimes also come fully formed in a single delta with
        // a `finish_reason` of `tool_calls`. When that happens the server
        // may not emit any further deltas, so we have nothing else to do
        // here — the [DONE] marker will arrive shortly and we'll flush.
        Ok(emitted)
    }

    fn flush_done_if_needed(&mut self) -> Option<StreamEvent> {
        if self.finished {
            return None;
        }
        self.finished = true;
        let mut out: Vec<ToolCall> = std::mem::take(&mut self.tool_calls);
        for (_, b) in self.builders.drain() {
            let input: Value =
                serde_json::from_str(&b.args).unwrap_or_else(|_| Value::String(b.args.clone()));
            out.push(ToolCall {
                id: b.id,
                name: b.name,
                input,
            });
        }
        Some(StreamEvent::Done(Completion {
            text: if self.text.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.text))
            },
            tool_calls: out,
        }))
    }
}

impl Stream for OpenAiSse {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(idx) = self.buffer.find('\n') {
                let line = self.buffer[..idx].to_string();
                self.buffer.drain(..=idx);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                match self.feed_line(line) {
                    Ok(Some(ev)) => return Poll::Ready(Some(Ok(ev))),
                    Ok(None) => continue,
                    Err(e) => return Poll::Ready(Some(Err(e))),
                }
            }
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                    continue;
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(anyhow::anyhow!("SSE read error: {e}"))));
                }
                Poll::Ready(None) => {
                    if let Some(ev) = self.flush_done_if_needed() {
                        return Poll::Ready(Some(Ok(ev)));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    fn chunk(data: &[u8]) -> std::result::Result<bytes::Bytes, reqwest::Error> {
        Ok(bytes::Bytes::copy_from_slice(data))
    }

    fn sse_events<S: Stream<Item = Result<StreamEvent>> + Unpin>(mut s: S) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        futures::executor::block_on(async {
            while let Some(ev) = s.next().await {
                out.push(ev.unwrap());
            }
        });
        out
    }

    fn byte_stream_from_strings(events: Vec<String>) -> ByteStream {
        Box::pin(futures::stream::iter(
            events
                .into_iter()
                .map(|s| chunk(s.as_bytes()))
                .collect::<Vec<_>>(),
        ))
    }

    #[test]
    fn anthropic_text_stream() {
        let events: Vec<String> = vec![
            "event: message_start\ndata: {\"type\":\"message_start\"}\n\n".into(),
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello \"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"world\"}}\n\n".into(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".into(),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".into(),
        ];
        // Build a minimal AnthropicSse directly (bypassing reqwest Response).
        let mut s = AnthropicSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            in_block: 0,
            cur_tool_id: String::new(),
            cur_tool_name: String::new(),
            cur_tool_input: String::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        // Expect 2 text deltas + 1 done.
        assert!(matches!(events[0], StreamEvent::TextDelta(ref s) if s == "hello "));
        assert!(matches!(events[1], StreamEvent::TextDelta(ref s) if s == "world"));
        match &events[2] {
            StreamEvent::Done(c) => {
                assert_eq!(c.text.as_deref(), Some("hello world"));
                assert!(c.tool_calls.is_empty());
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn anthropic_tool_use_stream() {
        let events: Vec<String> = vec![
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"bash\",\"input\":{}}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls\\\"}\"}}\n\n".into(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".into(),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".into(),
        ];
        let mut s = AnthropicSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            in_block: 0,
            cur_tool_id: String::new(),
            cur_tool_name: String::new(),
            cur_tool_input: String::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        // Expect one ToolUseBlock followed by Done.
        match &events[0] {
            StreamEvent::ToolUseBlock(tc) => {
                assert_eq!(tc.id, "call_1");
                assert_eq!(tc.name, "bash");
                assert_eq!(tc.input["command"], "ls");
            }
            other => panic!("expected ToolUseBlock, got {other:?}"),
        }
        match &events[1] {
            StreamEvent::Done(c) => {
                assert_eq!(c.tool_calls.len(), 1);
                assert_eq!(c.tool_calls[0].name, "bash");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn openai_text_stream() {
        let events: Vec<String> = vec![
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n"
                .into(),
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ];
        let mut s = OpenAiSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            builders: std::collections::HashMap::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        assert!(matches!(events[0], StreamEvent::TextDelta(ref s) if s == "hello "));
        assert!(matches!(events[1], StreamEvent::TextDelta(ref s) if s == "world"));
        match &events[2] {
            StreamEvent::Done(c) => assert_eq!(c.text.as_deref(), Some("hello world")),
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn openai_tool_call_stream() {
        let events: Vec<String> = vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ];
        let mut s = OpenAiSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            builders: std::collections::HashMap::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        match &events.last().unwrap() {
            StreamEvent::Done(c) => {
                assert_eq!(c.tool_calls.len(), 1);
                assert_eq!(c.tool_calls[0].name, "bash");
                assert_eq!(c.tool_calls[0].input["command"], "ls");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }
}

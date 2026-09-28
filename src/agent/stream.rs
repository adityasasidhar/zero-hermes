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

use std::collections::VecDeque;
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
        pending: VecDeque::new(),
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
    /// Events buffered for the consumer (one event per poll). The parser
    /// pushes here instead of returning inline so multiple events from a
    /// single SSE line (rare but possible) are preserved.
    pending: VecDeque<StreamEvent>,
    finished: bool,
}

impl AnthropicSse {
    /// Parse one SSE `data:` line and push any resulting events into
    /// `self.pending`.
    fn feed_line(&mut self, line: &str) -> Result<()> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let payload = data.trim();
        if payload.is_empty() {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            // Some providers ping heartbeat JSON we don't care about; skip
            // anything that doesn't parse rather than failing the stream.
            Err(e) => {
                tracing::debug!(error = %e, payload, "anthropic SSE: ignoring non-JSON line");
                return Ok(());
            }
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
            }
            "content_block_delta" => {
                let delta = v.get("delta").cloned().unwrap_or(Value::Null);
                let dtype = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match dtype {
                    "text_delta" => {
                        if let Some(t) = delta.get("text").and_then(|x| x.as_str()) {
                            self.text.push_str(t);
                            self.pending
                                .push_back(StreamEvent::TextDelta(t.to_string()));
                        }
                    }
                    "input_json_delta" => {
                        if let Some(partial) = delta.get("partial_json").and_then(|x| x.as_str()) {
                            self.cur_tool_input.push_str(partial);
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if self.in_block == 2 {
                    let input = match serde_json::from_str(&self.cur_tool_input) {
                        Ok(v) => v,
                        Err(_) => Value::String(self.cur_tool_input.clone()),
                    };
                    let tc = ToolCall {
                        id: std::mem::take(&mut self.cur_tool_id),
                        name: std::mem::take(&mut self.cur_tool_name),
                        input,
                    };
                    self.tool_calls.push(tc.clone());
                    self.pending.push_back(StreamEvent::ToolUseBlock(tc));
                }
                self.in_block = 0;
            }
            "message_stop" => {
                self.finished = true;
                self.pending.push_back(StreamEvent::Done(Completion {
                    text: if self.text.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut self.text))
                    },
                    tool_calls: std::mem::take(&mut self.tool_calls),
                }));
            }
            // message_start / message_delta / ping etc — ignore.
            _ => {}
        }
        Ok(())
    }

    fn flush_done_if_needed(&mut self) {
        if self.finished {
            return;
        }
        // Stream ended without an explicit message_stop (rare but possible
        // for cancelled or truncated responses); still emit a Done.
        self.finished = true;
        self.pending.push_back(StreamEvent::Done(Completion {
            text: if self.text.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.text))
            },
            tool_calls: std::mem::take(&mut self.tool_calls),
        }));
    }
}

impl Stream for AnthropicSse {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Drain any events already parsed before going back to the wire.
            if let Some(ev) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(ev)));
            }

            // Try to drain any complete lines already in the buffer.
            if let Some(idx) = self.buffer.find('\n') {
                // Vec::split_off takes ownership of the tail [at, len) and
                // leaves [0, at) in self.buffer with no allocation. We then
                // swap so self.buffer holds the remainder and `line_buf`
                // holds the line (still ending in '\n', which we pop).
                let line_buf = self.buffer.split_off(idx + 1);
                let mut line_buf = line_buf;
                std::mem::swap(&mut self.buffer, &mut line_buf);
                line_buf.pop(); // drop the trailing '\n'
                let line = line_buf.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                self.feed_line(line)?;
                continue;
            }

            // Need more bytes from the underlying byte stream.
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    // String::from_utf8_lossy is zero-copy when the bytes are
                    // valid UTF-8 (the normal case); the only allocation is
                    // when copying into self.buffer below.
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(anyhow::anyhow!("SSE read error: {e}"))));
                }
                Poll::Ready(None) => {
                    self.flush_done_if_needed();
                    if let Some(ev) = self.pending.pop_front() {
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
        // argument fragments. The parser flushes builders into `tool_calls`
        // (and emits `ToolUseBlock` events) when a `finish_reason:"tool_calls"`
        // marker arrives, or defensively at `[DONE]`.
        builders: std::collections::HashMap::new(),
        pending: VecDeque::new(),
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
    pending: VecDeque<StreamEvent>,
    finished: bool,
}

impl OpenAiSse {
    /// Build the final `ToolCall` from an in-flight builder, falling back to
    /// `Value::String(args)` when the partial JSON doesn't parse.
    fn builder_to_call(b: OpenAiToolBuilder) -> ToolCall {
        let input = match serde_json::from_str(&b.args) {
            Ok(v) => v,
            Err(_) => Value::String(b.args),
        };
        ToolCall {
            id: b.id,
            name: b.name,
            input,
        }
    }

    /// Flush all in-flight builders, push the resulting `ToolCall`s to
    /// `self.tool_calls` (so `Done` carries them) and queue a
    /// `ToolUseBlock` event per tool call (sorted by index for stable
    /// ordering). Idempotent: callers ensure builders are non-empty.
    fn flush_builders(&mut self) {
        if self.builders.is_empty() {
            return;
        }
        let mut keys: Vec<usize> = self.builders.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            if let Some(b) = self.builders.remove(&k) {
                let tc = Self::builder_to_call(b);
                self.tool_calls.push(tc.clone());
                self.pending.push_back(StreamEvent::ToolUseBlock(tc));
            }
        }
    }

    /// Parse one SSE `data:` line and queue any resulting events.
    fn feed_line(&mut self, line: &str) -> Result<()> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let payload = data.trim();
        if payload.is_empty() {
            return Ok(());
        }
        if payload == "[DONE]" {
            self.finished = true;
            // Defensive: flush any builders that never received a
            // finish_reason. Some servers omit it.
            self.flush_builders();
            self.pending.push_back(StreamEvent::Done(Completion {
                text: if self.text.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.text))
                },
                tool_calls: std::mem::take(&mut self.tool_calls),
            }));
            return Ok(());
        }
        let v: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(error = %e, payload, "openai SSE: ignoring non-JSON line");
                return Ok(());
            }
        };
        let choice = v
            .get("choices")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
        if let Some(content) = delta.get("content").and_then(|x| x.as_str()) {
            if !content.is_empty() {
                self.text.push_str(content);
                self.pending
                    .push_back(StreamEvent::TextDelta(content.to_string()));
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
        // finish_reason arrives in the *same* choice object that contains
        // the delta. The OpenAI spec uses "tool_calls" to mark completion.
        if let Some(fr) = choice.get("finish_reason").and_then(|x| x.as_str()) {
            if fr == "tool_calls" {
                self.flush_builders();
            }
        }
        Ok(())
    }

    fn flush_done_if_needed(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.flush_builders();
        self.pending.push_back(StreamEvent::Done(Completion {
            text: if self.text.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.text))
            },
            tool_calls: std::mem::take(&mut self.tool_calls),
        }));
    }
}

impl Stream for OpenAiSse {
    type Item = Result<StreamEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(ev) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(ev)));
            }
            if let Some(idx) = self.buffer.find('\n') {
                let line_buf = self.buffer.split_off(idx + 1);
                let mut line_buf = line_buf;
                std::mem::swap(&mut self.buffer, &mut line_buf);
                line_buf.pop();
                let line = line_buf.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                self.feed_line(line)?;
                continue;
            }
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(anyhow::anyhow!("SSE read error: {e}"))));
                }
                Poll::Ready(None) => {
                    self.flush_done_if_needed();
                    if let Some(ev) = self.pending.pop_front() {
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
        let mut s = AnthropicSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            in_block: 0,
            cur_tool_id: String::new(),
            cur_tool_name: String::new(),
            cur_tool_input: String::new(),
            pending: VecDeque::new(),
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
            pending: VecDeque::new(),
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
            pending: VecDeque::new(),
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
    fn openai_tool_call_stream_with_finish_reason() {
        // The "real" OpenAI flow: tool-call deltas followed by a
        // finish_reason:"tool_calls" marker.
        let events: Vec<String> = vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ];
        let mut s = OpenAiSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            builders: std::collections::HashMap::new(),
            pending: VecDeque::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        // ToolUseBlock should fire before Done.
        match &events[0] {
            StreamEvent::ToolUseBlock(tc) => {
                assert_eq!(tc.name, "bash");
                assert_eq!(tc.input["command"], "ls");
            }
            other => panic!("expected ToolUseBlock, got {other:?}"),
        }
        match events.last().unwrap() {
            StreamEvent::Done(c) => {
                assert_eq!(c.tool_calls.len(), 1);
                assert_eq!(c.tool_calls[0].name, "bash");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn openai_tool_call_stream_flushes_on_done() {
        // Some servers don't emit finish_reason. The parser should still
        // surface tool calls at [DONE].
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
            pending: VecDeque::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        match events.last().unwrap() {
            StreamEvent::Done(c) => {
                assert_eq!(c.tool_calls.len(), 1);
                assert_eq!(c.tool_calls[0].name, "bash");
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn openai_multiple_tool_calls() {
        // Two tool calls flushed together when finish_reason arrives.
        let events: Vec<String> = vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}},{\"index\":1,\"id\":\"c2\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ];
        let mut s = OpenAiSse {
            inner: byte_stream_from_strings(events),
            buffer: String::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            builders: std::collections::HashMap::new(),
            pending: VecDeque::new(),
            finished: false,
        };
        let events = sse_events(&mut s);
        let blocks: Vec<&ToolCall> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::ToolUseBlock(tc) => Some(tc),
                _ => None,
            })
            .collect();
        assert_eq!(
            blocks.len(),
            2,
            "expected two ToolUseBlocks, got {events:?}"
        );
        assert_eq!(blocks[0].name, "bash");
        assert_eq!(blocks[1].name, "read");
        match events.last().unwrap() {
            StreamEvent::Done(c) => assert_eq!(c.tool_calls.len(), 2),
            other => panic!("expected Done, got {other:?}"),
        }
    }
}

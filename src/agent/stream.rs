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
        buffer: Vec::new(),
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
    /// Raw bytes of the current, not-yet-newline-terminated line.
    ///
    /// Deliberately `Vec<u8>` and not `String`: a network read can end in
    /// the middle of a multi-byte UTF-8 sequence, and decoding per-chunk
    /// with `from_utf8_lossy` would turn the halves into U+FFFD instead of
    /// joining them. Since `0x0A` can never occur inside a multi-byte
    /// sequence, splitting on the raw byte and decoding whole *lines* is
    /// both safe and lossless.
    buffer: Vec<u8>,
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
            // The protocol end marker is authoritative. Once it has been
            // seen the stream is over, whether or not the peer closes the
            // body — a keep-alive connection that never sends EOF used to
            // leave the consumer waiting forever.
            if self.finished {
                return Poll::Ready(None);
            }

            // Try to drain any complete lines already in the buffer.
            if let Some(idx) = self.buffer.iter().position(|b| *b == b'\n') {
                // Vec::split_off takes ownership of the tail [at, len) and
                // leaves [0, at) in self.buffer with no allocation. We then
                // swap so self.buffer holds the remainder and `line_buf`
                // holds the line (still ending in '\n', which we pop).
                let mut line_buf = self.buffer.split_off(idx + 1);
                std::mem::swap(&mut self.buffer, &mut line_buf);
                line_buf.pop(); // drop the trailing '\n'
                                // A complete line is a complete UTF-8 unit, so decoding here
                                // (rather than per chunk) cannot split a codepoint.
                let line = String::from_utf8_lossy(&line_buf);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                self.feed_line(line)?;
                continue;
            }

            // Need more bytes from the underlying byte stream.
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    // Buffer raw bytes; decoding waits for a full line.
                    self.buffer.extend_from_slice(&chunk);
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
        buffer: Vec::new(),
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
    /// Raw bytes of the current, not-yet-newline-terminated line; see
    /// [`AnthropicSse::buffer`] for why this is not a `String`.
    buffer: Vec<u8>,
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
            // See AnthropicSse::poll_next: `[DONE]` ends the stream even if
            // the peer leaves the body open.
            if self.finished {
                return Poll::Ready(None);
            }
            if let Some(idx) = self.buffer.iter().position(|b| *b == b'\n') {
                let mut line_buf = self.buffer.split_off(idx + 1);
                std::mem::swap(&mut self.buffer, &mut line_buf);
                line_buf.pop();
                // Decode a whole line at a time so a codepoint split across
                // two chunks survives.
                let line = String::from_utf8_lossy(&line_buf);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                self.feed_line(line)?;
                continue;
            }
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    self.buffer.extend_from_slice(&chunk);
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
    use std::time::Duration;

    fn chunk(data: &[u8]) -> std::result::Result<bytes::Bytes, reqwest::Error> {
        Ok(bytes::Bytes::copy_from_slice(data))
    }

    /// Build the private parser state around an arbitrary byte stream.
    fn anthropic_with(inner: ByteStream) -> AnthropicSse {
        AnthropicSse {
            inner,
            buffer: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            in_block: 0,
            cur_tool_id: String::new(),
            cur_tool_name: String::new(),
            cur_tool_input: String::new(),
            pending: VecDeque::new(),
            finished: false,
        }
    }

    fn openai_with(inner: ByteStream) -> OpenAiSse {
        OpenAiSse {
            inner,
            buffer: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            builders: std::collections::HashMap::new(),
            pending: VecDeque::new(),
            finished: false,
        }
    }

    /// A byte stream that yields `chunks` verbatim, then ends.
    fn stream_of(chunks: Vec<Vec<u8>>) -> ByteStream {
        Box::pin(futures::stream::iter(
            chunks.into_iter().map(|c| chunk(&c)).collect::<Vec<_>>(),
        ))
    }

    /// A byte stream that yields `chunks` and then stays open forever — what
    /// a keep-alive HTTP body looks like when it never sends EOF.
    fn stream_of_then_open(chunks: Vec<Vec<u8>>) -> ByteStream {
        Box::pin(
            futures::stream::iter(chunks.into_iter().map(|c| chunk(&c)).collect::<Vec<_>>())
                .chain(futures::stream::pending()),
        )
    }

    fn byte_stream_from_strings(events: Vec<String>) -> ByteStream {
        stream_of(events.into_iter().map(String::into_bytes).collect())
    }

    /// Project events onto a comparable shape (`StreamEvent` has no `PartialEq`).
    fn shape(events: &[StreamEvent]) -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                StreamEvent::TextDelta(t) => format!("text:{t}"),
                StreamEvent::ToolUseBlock(tc) => {
                    format!("tool:{}:{}:{}", tc.id, tc.name, tc.input)
                }
                StreamEvent::Done(c) => format!(
                    "done:{}:{}",
                    c.text.clone().unwrap_or_default(),
                    c.tool_calls
                        .iter()
                        .map(|t| format!("{}={}", t.name, t.input))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            })
            .collect()
    }

    /// Drain a stream on the current (single) thread. Used by the sync tests.
    fn sse_events<S: Stream<Item = Result<StreamEvent>> + Unpin>(mut s: S) -> Vec<StreamEvent> {
        let mut out = Vec::new();
        futures::executor::block_on(async {
            while let Some(ev) = s.next().await {
                out.push(ev.unwrap());
            }
        });
        out
    }

    /// Drain a stream, failing loudly instead of hanging if it never ends.
    async fn drain_with_deadline<S>(mut s: S) -> Vec<StreamEvent>
    where
        S: Stream<Item = Result<StreamEvent>> + Unpin,
    {
        let mut out = Vec::new();
        loop {
            match tokio::time::timeout(Duration::from_secs(30), s.next()).await {
                Ok(Some(ev)) => out.push(ev.expect("stream error")),
                Ok(None) => return out,
                Err(_) => panic!("stream never terminated; got {out:?}"),
            }
        }
    }

    // The reference payloads below deliberately contain non-ASCII text.
    // Real model output is multilingual, and a multi-byte character landing
    // on a chunk boundary is the single most likely way for the parser to
    // corrupt an answer.
    const ANTHROPIC_TEXT_FRAMES: [&str; 5] = [
        "event: message_start\ndata: {\"type\":\"message_start\"}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Ünïcøde €100 🎉 ✓ nihongo 日本語\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];

    const ANTHROPIC_TOOL_FRAMES: [&str; 5] = [
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"bash\",\"input\":{}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls 🎉\\\"}\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];

    const OPENAI_TEXT_FRAMES: [&str; 4] = [
        "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Ünïcøde €100 🎉\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" ✓ 日本語\"}}]}\n\n",
        "data: [DONE]\n\n",
    ];

    const OPENAI_TOOL_FRAMES: [&str; 4] = [
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls 🎉\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n",
        "data: [DONE]\n\n",
    ];

    fn transcript(frames: &[&str]) -> Vec<u8> {
        frames.concat().into_bytes()
    }

    // -----------------------------------------------------------------------
    // Chunk-boundary handling
    // -----------------------------------------------------------------------
    //
    // Every test in this section feeds the *same* bytes as the happy-path
    // tests above, just cut into pieces at a different place. A parser that
    // only works when a frame arrives in one piece is broken against any
    // real network.

    /// The exact regressions this guards: before the fix, both parsers
    /// decoded each network chunk independently with
    /// `String::from_utf8_lossy`, so a multi-byte character straddling a
    /// chunk boundary became two U+FFFD replacement characters.
    #[test]
    fn anthropic_text_is_identical_under_every_two_way_byte_split() {
        let bytes = transcript(&ANTHROPIC_TEXT_FRAMES);
        let reference = shape(&sse_events(anthropic_with(stream_of(vec![bytes.clone()]))));
        assert_eq!(
            reference,
            vec![
                "text:Ünïcøde €100 🎉 ✓ nihongo 日本語".to_string(),
                "done:Ünïcøde €100 🎉 ✓ nihongo 日本語:".to_string(),
            ],
            "the unsplit reference run should see the deltas verbatim"
        );

        for split in 1..bytes.len() {
            let chunks = vec![bytes[..split].to_vec(), bytes[split..].to_vec()];
            let got = shape(&sse_events(anthropic_with(stream_of(chunks))));
            assert_eq!(
                got, reference,
                "splitting the body at byte {split} changed the parsed stream"
            );
        }
    }

    #[test]
    fn openai_text_is_identical_under_every_two_way_byte_split() {
        let bytes = transcript(&OPENAI_TEXT_FRAMES);
        let reference = shape(&sse_events(openai_with(stream_of(vec![bytes.clone()]))));
        assert_eq!(
            reference,
            vec![
                "text:Ünïcøde €100 🎉".to_string(),
                "text: ✓ 日本語".to_string(),
                "done:Ünïcøde €100 🎉 ✓ 日本語:".to_string(),
            ]
        );

        for split in 1..bytes.len() {
            let chunks = vec![bytes[..split].to_vec(), bytes[split..].to_vec()];
            let got = shape(&sse_events(openai_with(stream_of(chunks))));
            assert_eq!(
                got, reference,
                "splitting the body at byte {split} changed the parsed stream"
            );
        }
    }

    /// The pathological case: one byte per read. Exercises a boundary at
    /// every offset at once, including inside the JSON escapes and inside
    /// four-byte codepoints.
    #[test]
    fn parsers_survive_byte_at_a_time_delivery() {
        let bytes = transcript(&ANTHROPIC_TEXT_FRAMES);
        let whole = shape(&sse_events(anthropic_with(stream_of(vec![bytes.clone()]))));
        let bytewise = shape(&sse_events(anthropic_with(stream_of(
            bytes.iter().map(|b| vec![*b]).collect(),
        ))));
        assert_eq!(bytewise, whole, "anthropic parser, 1 byte per chunk");

        let bytes = transcript(&OPENAI_TEXT_FRAMES);
        let whole = shape(&sse_events(openai_with(stream_of(vec![bytes.clone()]))));
        let bytewise = shape(&sse_events(openai_with(stream_of(
            bytes.iter().map(|b| vec![*b]).collect(),
        ))));
        assert_eq!(bytewise, whole, "openai parser, 1 byte per chunk");
    }

    #[test]
    fn anthropic_tool_call_is_identical_under_every_two_way_byte_split() {
        let bytes = transcript(&ANTHROPIC_TOOL_FRAMES);
        let reference = shape(&sse_events(anthropic_with(stream_of(vec![bytes.clone()]))));
        assert!(
            reference[0].contains("ls 🎉"),
            "reference run should carry the tool argument: {reference:?}"
        );
        for split in 1..bytes.len() {
            let chunks = vec![bytes[..split].to_vec(), bytes[split..].to_vec()];
            let got = shape(&sse_events(anthropic_with(stream_of(chunks))));
            assert_eq!(
                got, reference,
                "splitting the body at byte {split} changed the tool call"
            );
        }
    }

    #[test]
    fn openai_tool_call_is_identical_under_every_two_way_byte_split() {
        let bytes = transcript(&OPENAI_TOOL_FRAMES);
        let reference = shape(&sse_events(openai_with(stream_of(vec![bytes.clone()]))));
        assert!(
            reference[0].contains("ls 🎉"),
            "reference run should carry the tool argument: {reference:?}"
        );
        for split in 1..bytes.len() {
            let chunks = vec![bytes[..split].to_vec(), bytes[split..].to_vec()];
            let got = shape(&sse_events(openai_with(stream_of(chunks))));
            assert_eq!(
                got, reference,
                "splitting the body at byte {split} changed the tool call"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Line framing
    // -----------------------------------------------------------------------

    #[test]
    fn anthropic_accepts_crlf_line_endings() {
        // SSE permits CRLF; a proxy or a Windows-based shim will use it.
        let frames: Vec<String> = ANTHROPIC_TEXT_FRAMES
            .iter()
            .map(|f| f.replace('\n', "\r\n"))
            .collect();
        let events = sse_events(anthropic_with(byte_stream_from_strings(frames)));
        assert_eq!(
            shape(&events),
            vec![
                "text:Ünïcøde €100 🎉 ✓ nihongo 日本語".to_string(),
                "done:Ünïcøde €100 🎉 ✓ nihongo 日本語:".to_string(),
            ]
        );
    }

    #[test]
    fn openai_accepts_crlf_line_endings() {
        let frames: Vec<String> = OPENAI_TEXT_FRAMES
            .iter()
            .map(|f| f.replace('\n', "\r\n"))
            .collect();
        let events = sse_events(openai_with(byte_stream_from_strings(frames)));
        assert_eq!(
            shape(&events),
            vec![
                "text:Ünïcøde €100 🎉".to_string(),
                "text: ✓ 日本語".to_string(),
                "done:Ünïcøde €100 🎉 ✓ 日本語:".to_string(),
            ]
        );
    }

    #[test]
    fn non_data_and_unparseable_lines_are_skipped() {
        // Providers interleave `event:`/`id:`/`retry:` lines with keep-alive
        // comments and the occasional malformed payload. None of it may fail
        // the stream or leak into the user's answer.
        let frames = vec![
            ": keep-alive comment\n\n".to_string(),
            "event: message_start\ndata: not-json-at-all\n\n".to_string(),
            "id: 42\n".to_string(),
            "data: {\"type\":\"ping\"}\n\n".to_string(),
            ANTHROPIC_TEXT_FRAMES[2].to_string(),
            "data: {\"unterminated\":\n\n".to_string(),
            ANTHROPIC_TEXT_FRAMES[4].to_string(),
        ];
        let events = sse_events(anthropic_with(byte_stream_from_strings(frames)));
        assert_eq!(
            shape(&events),
            vec![
                "text:Ünïcøde €100 🎉 ✓ nihongo 日本語".to_string(),
                "done:Ünïcøde €100 🎉 ✓ nihongo 日本語:".to_string(),
            ]
        );
    }

    #[test]
    fn openai_skips_unparseable_lines() {
        let frames = vec![
            ": keep-alive\n\n".to_string(),
            "data: {\"choices\":oops\n\n".to_string(),
            OPENAI_TEXT_FRAMES[1].to_string(),
            "data: [DONE]\n\n".to_string(),
        ];
        let events = sse_events(openai_with(byte_stream_from_strings(frames)));
        assert_eq!(
            shape(&events),
            vec![
                "text:Ünïcøde €100 🎉".to_string(),
                "done:Ünïcøde €100 🎉:".to_string(),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Termination
    // -----------------------------------------------------------------------

    /// A stream that is cut off before its end marker (dropped connection,
    /// cancelled request) must still produce a `Done`, otherwise the agent
    /// loop waits on a stream that will never yield a completion.
    #[test]
    fn truncated_streams_still_emit_done() {
        // message_start + content_block_start + one delta, then the
        // connection drops before message_stop.
        let frames: Vec<String> = ANTHROPIC_TEXT_FRAMES[..3]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let events = sse_events(anthropic_with(byte_stream_from_strings(frames)));
        match events.last() {
            Some(StreamEvent::Done(c)) => {
                assert_eq!(c.text.as_deref(), Some("Ünïcøde €100 🎉 ✓ nihongo 日本語"));
            }
            other => panic!("expected a trailing Done, got {other:?}"),
        }

        let frames: Vec<String> = OPENAI_TEXT_FRAMES[..3]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let events = sse_events(openai_with(byte_stream_from_strings(frames)));
        match events.last() {
            Some(StreamEvent::Done(c)) => {
                assert_eq!(c.text.as_deref(), Some("Ünïcøde €100 🎉 ✓ 日本語"));
            }
            other => panic!("expected a trailing Done, got {other:?}"),
        }
    }

    /// The end marker is authoritative: a peer that keeps the body open
    /// after `message_stop` / `[DONE]` used to strand the consumer, because
    /// the parser only finished when the underlying body reported EOF.
    #[tokio::test(start_paused = true)]
    async fn anthropic_finishes_on_message_stop_even_if_the_body_stays_open() {
        let s = anthropic_with(stream_of_then_open(vec![transcript(
            &ANTHROPIC_TEXT_FRAMES,
        )]));
        let events = drain_with_deadline(s).await;
        assert!(matches!(events.last(), Some(StreamEvent::Done(_))));
        assert_eq!(shape(&events).len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn openai_finishes_on_done_marker_even_if_the_body_stays_open() {
        let s = openai_with(stream_of_then_open(vec![transcript(&OPENAI_TEXT_FRAMES)]));
        let events = drain_with_deadline(s).await;
        assert!(matches!(events.last(), Some(StreamEvent::Done(_))));
        assert_eq!(shape(&events).len(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn anthropic_finishes_when_the_body_ends_without_any_end_marker() {
        // Only the first three frames: no message_stop, then EOF.
        let s = anthropic_with(stream_of(vec![transcript(&ANTHROPIC_TEXT_FRAMES[..3])]));
        let events = drain_with_deadline(s).await;
        assert!(matches!(events.last(), Some(StreamEvent::Done(_))));
    }

    // -----------------------------------------------------------------------
    // Happy paths (kept from the original suite, now built via the helpers)
    // -----------------------------------------------------------------------

    #[test]
    fn anthropic_text_stream() {
        let events = sse_events(anthropic_with(byte_stream_from_strings(vec![
            "event: message_start\ndata: {\"type\":\"message_start\"}\n\n".into(),
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello \"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"world\"}}\n\n".into(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".into(),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".into(),
        ])));
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
        let events = sse_events(anthropic_with(byte_stream_from_strings(vec![
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"bash\",\"input\":{}}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\"}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls\\\"}\"}}\n\n".into(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".into(),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".into(),
        ])));
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

    /// A tool call whose partial JSON never becomes valid (truncated stream)
    /// must still reach the model as *something*, rather than being dropped.
    #[test]
    fn anthropic_truncated_tool_json_falls_back_to_raw_string() {
        let events = sse_events(anthropic_with(byte_stream_from_strings(vec![
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"bash\",\"input\":{}}}\n\n".into(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\": \\\"ls\"}}\n\n".into(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".into(),
        ])));
        match &events[0] {
            StreamEvent::ToolUseBlock(tc) => {
                assert_eq!(tc.name, "bash");
                assert_eq!(tc.input, serde_json::json!("{\"command\": \"ls"));
            }
            other => panic!("expected ToolUseBlock, got {other:?}"),
        }
    }

    #[test]
    fn openai_text_stream() {
        let events = sse_events(openai_with(byte_stream_from_strings(vec![
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n"
                .into(),
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello \"}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ])));
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
        let events = sse_events(openai_with(byte_stream_from_strings(vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ])));
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
        let events = sse_events(openai_with(byte_stream_from_strings(vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ])));
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
        let events = sse_events(openai_with(byte_stream_from_strings(vec![
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}},{\"index\":1,\"id\":\"c2\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"\"}}]}}]}\n\n".into(),
            "data: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"delta\":{}}]}\n\n".into(),
            "data: [DONE]\n\n".into(),
        ])));
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

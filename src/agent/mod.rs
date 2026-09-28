//! The agent loop.
//!
//! `run` is the heart of the project: given a provider, a tool registry,
//! a system prompt and a conversation history, it sends the messages
//! to the LLM, dispatches any tool calls, collects results, and loops
//! until the model produces a final text answer or hits the iteration
//! cap.

pub mod context;
pub mod mock;
pub mod provider;
pub mod stream;
pub mod tool;

use std::sync::Arc;

pub use crate::agent::context::compact_history;
pub use crate::agent::provider::{
    build_provider, AnthropicMessages, Completion, LlmProvider, OpenAiCompat,
};
pub use crate::agent::stream::{EventStream, StreamEvent};
pub use crate::agent::tool::{
    ContentBlock, Message, Tool, ToolCall, ToolContext, ToolOutput, ToolResult,
};

use crate::config::AgentConfig;
use crate::error::Result;
use crate::tools::ToolRegistry;

/// Remove provider reasoning enclosed in MiniMax-style `<think>` tags.
///
/// Some OpenAI-compatible providers expose reasoning as ordinary text rather
/// than a separate response field. Keep it out of the user-visible answer and
/// conversation history. The streaming form below handles tags split across
/// arbitrary SSE chunks.
fn strip_think_blocks(text: &str) -> String {
    let mut filter = ThinkFilter::default();
    let mut visible = filter.push(text);
    visible.push_str(&filter.finish());
    visible
}

/// Incremental counterpart to [`strip_think_blocks`].
#[derive(Default)]
struct ThinkFilter {
    in_think: bool,
    pending: String,
}

impl ThinkFilter {
    /// Consume a provider text chunk and return only text safe to display.
    fn push(&mut self, chunk: &str) -> String {
        const OPEN: &str = "<think>";
        const CLOSE: &str = "</think>";

        self.pending.push_str(chunk);
        let mut visible = String::new();

        loop {
            if self.in_think {
                if let Some(end) = self.pending.find(CLOSE) {
                    self.pending.drain(..end + CLOSE.len());
                    self.in_think = false;
                    continue;
                }
                // Keep all reasoning until its closing tag arrives. It is
                // intentionally never forwarded to the caller.
                break;
            }

            if let Some(start) = self.pending.find(OPEN) {
                visible.push_str(&self.pending[..start]);
                self.pending.drain(..start + OPEN.len());
                self.in_think = true;
                continue;
            }

            // Do not emit a possible beginning of `<think>` yet: an SSE
            // chunk may end midway through the tag.
            let keep = (1..OPEN.len())
                .rev()
                .find(|&n| self.pending.ends_with(&OPEN[..n]))
                .unwrap_or(0);
            let emit_len = self.pending.len() - keep;
            visible.push_str(&self.pending[..emit_len]);
            self.pending.drain(..emit_len);
            break;
        }

        visible
    }

    /// Finish a response. Unterminated reasoning is discarded; an incomplete
    /// non-reasoning tag fragment is regular text and is preserved.
    fn finish(&mut self) -> String {
        if self.in_think {
            self.pending.clear();
            self.in_think = false;
            String::new()
        } else {
            std::mem::take(&mut self.pending)
        }
    }
}

/// Maximum iterations the outer loop will run before giving up.
pub const DEFAULT_MAX_ITERATIONS: usize = 50;

/// Whether a provider error looks transient and is worth retrying.
///
/// Matches on the error text for rate-limit / overload / 5xx / timeout
/// signals. Tool errors never reach this path — only `llm.complete` /
/// stream-establishment failures are retried.
fn is_transient_provider_error(e: &anyhow::Error) -> bool {
    let s = e.to_string().to_lowercase();
    const MARKERS: &[&str] = &[
        "rate limit",
        "ratelimit",
        "429",
        "500",
        "502",
        "503",
        "504",
        "overload",
        "temporar",
        "try again",
        "service unavailable",
        "timeout",
        "timed out",
        "connection",
        "network",
        "eof",
        "broken pipe",
        "reset by peer",
    ];
    MARKERS.iter().any(|m| s.contains(m))
}

/// Backoff between provider retries (200ms, 800ms, 2s): up to 3 retries.
const RETRY_BACKOFFS_MS: [u64; 3] = [200, 800, 2000];

/// Call `llm.complete`, retrying transient provider errors up to 3 times
/// with exponential backoff. Tool errors are never retried — they surface
/// through [`dispatch_tool`], not this path.
async fn complete_with_retry<P: LlmProvider + ?Sized>(
    llm: &P,
    system: Option<&str>,
    history: &[Message],
    tool_schemas: &[serde_json::Value],
) -> Result<Completion> {
    let mut attempt = 0usize;
    loop {
        match llm.complete(system, history, tool_schemas).await {
            Ok(c) => return Ok(c),
            Err(e) => {
                if attempt >= RETRY_BACKOFFS_MS.len() || !is_transient_provider_error(&e) {
                    return Err(e);
                }
                tokio::time::sleep(std::time::Duration::from_millis(RETRY_BACKOFFS_MS[attempt]))
                    .await;
                attempt += 1;
            }
        }
    }
}

/// Dispatch one round of tool calls: read-only tools run concurrently,
/// mutating tools run sequentially in input order. Results are returned
/// in input order so the caller can zip them back with the requests.
async fn dispatch_tool_calls(
    tools: &ToolRegistry,
    calls: &[ToolCall],
    ctx: &ToolContext,
) -> Vec<ToolResult> {
    use std::collections::HashMap;
    let readonly: HashMap<usize, bool> = calls
        .iter()
        .enumerate()
        .map(|(i, tc)| {
            let ro = tools.get(&tc.name).is_some_and(|t| t.is_read_only());
            (i, ro)
        })
        .collect();
    let ro_indices: Vec<usize> = calls
        .iter()
        .enumerate()
        .filter(|(i, _)| readonly[i])
        .map(|(i, _)| i)
        .collect();
    // Concurrent batch for the side-effect-free calls.
    let ro_results = futures::future::join_all(
        ro_indices
            .iter()
            .map(|&i| dispatch_tool(tools, &calls[i], ctx)),
    )
    .await;
    let mut out: Vec<Option<ToolResult>> = calls.iter().map(|_| None).collect();
    for (&idx, res) in ro_indices.iter().zip(ro_results) {
        out[idx] = Some(res);
    }
    // Mutating calls (and unknown tools) run one at a time, in order.
    for (i, tc) in calls.iter().enumerate() {
        if out[i].is_none() {
            out[i] = Some(dispatch_tool(tools, tc, ctx).await);
        }
    }
    // Every slot is filled above, but a missing slot must never panic the
    // daemon (release sets `panic = "abort"`): degrade to an error result
    // naming the unmatched call instead.
    out.into_iter()
        .zip(calls.iter())
        .map(|(r, tc)| {
            r.unwrap_or_else(|| ToolResult::err(&tc.id, "internal error: missing tool result"))
        })
        .collect()
}

/// Repair `history` after a failed [`run`] / [`run_stream`] turn.
///
/// `run` pushes the user message before the first LLM call, so a provider
/// error leaves an orphan trailing user turn that would make the next turn
/// start with `user,user`. Pop it — but only when it is a plain-text user
/// message. A trailing `tool_result` message must be kept: popping it
/// would orphan the preceding assistant `tool_use` and corrupt the
/// transcript worse than the duplicate role would.
pub fn repair_history_after_failure(history: &mut Vec<Message>) -> bool {
    let Some(last) = history.last() else {
        return false;
    };
    if last.role != "user" {
        return false;
    }
    let has_result = last
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
    if has_result {
        return false;
    }
    history.pop();
    true
}

/// Default number of messages retained before compaction kicks in.
pub const DEFAULT_CONTEXT_WINDOW: usize = 50;
/// Default approximate token budget retained before history is summarized.
pub const DEFAULT_CONTEXT_TOKENS: usize = 24_000;

/// Per-turn bounds for the agent loop.
///
/// Bundled into one struct rather than passed as two more positional
/// arguments — `run` already takes enough of them, and the two limits are
/// always configured together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunLimits {
    /// Maximum LLM/tool round-trips for a single user turn.
    pub max_iterations: usize,
    /// Maximum messages retained in `history` before the oldest are
    /// dropped. See [`compact_history`]. `0` or `usize::MAX` disables it.
    pub context_window: usize,
    /// Approximate transcript-token limit; a conservative local estimate.
    pub context_tokens: usize,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            context_window: DEFAULT_CONTEXT_WINDOW,
            context_tokens: DEFAULT_CONTEXT_TOKENS,
        }
    }
}

impl RunLimits {
    /// Cap iterations, leaving the context window at its default. Mostly
    /// useful in tests and for bounded background work like cron ticks.
    pub fn iterations(max_iterations: usize) -> Self {
        Self {
            max_iterations,
            ..Self::default()
        }
    }

    /// Override the context window, keeping the iteration cap.
    pub fn with_context_window(mut self, context_window: usize) -> Self {
        self.context_window = context_window;
        self
    }
}

impl From<&AgentConfig> for RunLimits {
    fn from(cfg: &AgentConfig) -> Self {
        Self {
            max_iterations: cfg.max_iterations,
            context_window: cfg.context_window,
            context_tokens: cfg.context_tokens,
        }
    }
}

/// Run the agent loop for a single user turn.
///
/// 1. Append `user` to `history`.
/// 2. Loop up to `max_iterations` times: call `llm.complete`. If the model
///    returned text and no tool calls, append the assistant message and
///    return the text. If the model returned tool calls, dispatch them via
///    the registry, append the assistant message + tool results, continue.
/// 3. On iteration overflow, return whatever text we have (or an error).
#[allow(clippy::too_many_arguments)]
pub async fn run<P: LlmProvider + ?Sized>(
    llm: &P,
    tools: &ToolRegistry,
    system: Option<&str>,
    history: &mut Vec<Message>,
    user: &str,
    limits: RunLimits,
    ctx: &ToolContext,
) -> Result<String> {
    history.push(Message::user(user));
    let tool_schemas = tools.schemas();
    let mut last_text: Option<String> = None;

    for iteration in 0..limits.max_iterations {
        tracing::debug!(iteration, "agent loop iteration");
        compact_history(history, limits.context_window);
        context::compact_history_to_tokens(history, limits.context_tokens);
        let mut completion = complete_with_retry(llm, system, history, &tool_schemas).await?;
        completion.text = completion.text.map(|text| strip_think_blocks(&text));

        if completion.tool_calls.is_empty() {
            let text = completion.text.unwrap_or_default();
            history.push(Message::assistant_text(&text));
            return Ok(text);
        }

        let mut blocks: Vec<ContentBlock> = Vec::with_capacity(1 + completion.tool_calls.len());
        // Remember the partial text *before* moving it into the block list;
        // it is the best answer we have if the loop later runs out of
        // iterations.
        if let Some(text) = completion.text.take() {
            if !text.is_empty() {
                last_text = Some(text.clone());
                blocks.push(ContentBlock::Text { text });
            }
        }
        for tc in &completion.tool_calls {
            blocks.push(ContentBlock::ToolUse(tc.clone()));
        }
        history.push(Message::assistant(blocks));

        let results = dispatch_tool_calls(tools, &completion.tool_calls, ctx).await;
        history.push(Message::tool_results(results));
    }

    let fallback = last_text.unwrap_or_else(|| {
        "(agent loop hit max iterations without producing a final answer)".to_string()
    });
    history.push(Message::assistant_text(&fallback));
    Ok(fallback)
}

/// A single event yielded by [`run_stream`] for each LLM round-trip.
#[derive(Debug, Clone)]
pub enum StreamTurn {
    /// Text delta from the model (during the current iteration).
    TextDelta(String),
    /// The model just emitted a tool invocation request; the LLM-produced
    /// `ToolCall` is included.
    ToolUse(ToolCall),
    /// A dispatched tool finished. Emitted after the tool runs so a UI can
    /// show what came back, not just what was asked for.
    ToolResult {
        /// Name of the tool that ran.
        name: String,
        /// Whatever the tool returned (or the error text on failure).
        output: String,
        /// Whether the tool reported failure.
        is_error: bool,
    },
    /// The loop finished (either the model returned final text or it hit
    /// the iteration cap). The string is the final text answer.
    Done(String),
}

/// Streaming variant of [`run`]. Drives the same loop but calls
/// `llm.stream(...)` and forwards each event through `on_event` as it
/// arrives. Returns the same final string as `run`.
#[allow(clippy::too_many_arguments)]
pub async fn run_stream<P: LlmProvider + ?Sized, F>(
    llm: &P,
    tools: &ToolRegistry,
    system: Option<&str>,
    history: &mut Vec<Message>,
    user: &str,
    limits: RunLimits,
    ctx: &ToolContext,
    mut on_event: F,
) -> Result<String>
where
    F: FnMut(StreamTurn) + Send,
{
    use futures::StreamExt;

    history.push(Message::user(user));
    let tool_schemas = tools.schemas();
    let mut last_text: Option<String> = None;

    for iteration in 0..limits.max_iterations {
        tracing::debug!(iteration, "agent loop iteration (stream)");
        compact_history(history, limits.context_window);
        context::compact_history_to_tokens(history, limits.context_tokens);

        // Consume the stream fully inside this scope so the immutable
        // borrow of `history` (and the other refs) ends before we
        // push to it below. Stream establishment is retried up to 3
        // times on transient errors — but only while nothing has been
        // emitted yet, so the UI never sees duplicated deltas.
        let (completion, current_text) = {
            let mut attempt = 0usize;
            loop {
                let mut stream = llm.stream(system, history, &tool_schemas);

                let mut current_text = String::new();
                let mut think_filter = ThinkFilter::default();
                let mut event: Option<StreamEvent> = None;
                let mut emitted_any = false;
                let mut stream_err: Option<anyhow::Error> = None;

                while let Some(item) = stream.next().await {
                    let ev = match item {
                        Ok(ev) => ev,
                        Err(e) => {
                            stream_err = Some(e);
                            break;
                        }
                    };
                    match ev {
                        StreamEvent::TextDelta(s) => {
                            let visible = think_filter.push(&s);
                            if !visible.is_empty() {
                                on_event(StreamTurn::TextDelta(visible.clone()));
                                emitted_any = true;
                                current_text.push_str(&visible);
                            }
                        }
                        StreamEvent::ToolUseBlock(tc) => {
                            on_event(StreamTurn::ToolUse(tc));
                            emitted_any = true;
                        }
                        StreamEvent::Done(mut completion) => {
                            let visible = think_filter.finish();
                            if !visible.is_empty() {
                                on_event(StreamTurn::TextDelta(visible.clone()));
                                emitted_any = true;
                                current_text.push_str(&visible);
                            }
                            completion.text =
                                (!current_text.is_empty()).then(|| current_text.clone());
                            event = Some(StreamEvent::Done(completion));
                            break;
                        }
                    }
                }
                if let Some(e) = stream_err {
                    if !emitted_any
                        && attempt < RETRY_BACKOFFS_MS.len()
                        && is_transient_provider_error(&e)
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            RETRY_BACKOFFS_MS[attempt],
                        ))
                        .await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e);
                }
                let Some(StreamEvent::Done(completion)) = event else {
                    anyhow::bail!("LLM stream ended without a Done event");
                };
                break (completion, current_text);
            }
        };

        if completion.tool_calls.is_empty() {
            let text = completion.text.unwrap_or_default();
            history.push(Message::assistant_text(&text));
            on_event(StreamTurn::Done(text.clone()));
            return Ok(text);
        }

        // Build assistant message from the streamed tool calls.
        let mut blocks: Vec<ContentBlock> = Vec::with_capacity(1 + completion.tool_calls.len());
        if !current_text.is_empty() {
            blocks.push(ContentBlock::Text { text: current_text });
        }
        for tc in &completion.tool_calls {
            blocks.push(ContentBlock::ToolUse(tc.clone()));
        }
        history.push(Message::assistant(blocks));
        if let Some(text) = completion.text {
            if !text.is_empty() {
                last_text = Some(text);
            }
        }

        let results = dispatch_tool_calls(tools, &completion.tool_calls, ctx).await;
        for (tc, result) in completion.tool_calls.iter().zip(results.iter()) {
            on_event(StreamTurn::ToolResult {
                name: tc.name.clone(),
                output: result.content.clone(),
                is_error: result.is_error,
            });
        }
        history.push(Message::tool_results(results));
    }

    let fallback = last_text.unwrap_or_else(|| {
        "(agent loop hit max iterations without producing a final answer)".to_string()
    });
    history.push(Message::assistant_text(&fallback));
    on_event(StreamTurn::Done(fallback.clone()));
    Ok(fallback)
}

/// Dispatch a single tool call. Tool errors are converted to `is_error: true`
/// results so the model can recover.
pub async fn dispatch_tool(tools: &ToolRegistry, call: &ToolCall, ctx: &ToolContext) -> ToolResult {
    match tools.call(&call.name, call.input.clone(), ctx).await {
        // Builtins report failure as `Ok(ToolOutput::err(..))` rather than
        // `Err`, so the flag has to be carried across — dropping it told
        // the model that every failed command had succeeded.
        Ok(out) if out.is_error => ToolResult::err(&call.id, out.content),
        Ok(out) => ToolResult::ok(&call.id, out.content),
        Err(e) => ToolResult::err(&call.id, format!("tool error: {e}")),
    }
}

/// Build a `ToolContext` pre-wired with memory.
pub fn make_context_with_memory(
    cwd: Option<std::path::PathBuf>,
    memory: Arc<crate::memory::Memory>,
) -> ToolContext {
    ToolContext {
        cwd,
        session_id: None,
        memory: Some(memory),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_complete_think_blocks() {
        assert_eq!(
            strip_think_blocks("<think>private reasoning</think>Visible answer"),
            "Visible answer"
        );
    }

    #[test]
    fn think_filter_handles_tags_split_across_chunks() {
        let mut filter = ThinkFilter::default();
        assert_eq!(filter.push("before <thi"), "before ");
        assert_eq!(filter.push("nk>private"), "");
        assert_eq!(filter.push(" reasoning</th"), "");
        assert_eq!(filter.push("ink> after"), " after");
        assert_eq!(filter.finish(), "");
    }

    #[test]
    fn unterminated_thinking_is_not_shown() {
        assert_eq!(strip_think_blocks("answer<think>private"), "answer");
    }

    #[test]
    fn transient_markers_catch_rate_limits_and_5xx() {
        for msg in [
            "LLM returned 429: rate limit exceeded",
            "LLM returned 503: overloaded",
            "LLM request failed: timeout",
            "service unavailable, try again later",
        ] {
            assert!(
                is_transient_provider_error(&anyhow::anyhow!(msg)),
                "{msg} should be transient"
            );
        }
        assert!(!is_transient_provider_error(&anyhow::anyhow!(
            "tool_use without id"
        )));
        assert!(!is_transient_provider_error(&anyhow::anyhow!(
            "unknown tool: nope"
        )));
    }

    #[test]
    fn repair_pops_orphan_plain_user_message() {
        let mut h = vec![Message::user("earlier"), Message::assistant_text("reply")];
        assert!(!repair_history_after_failure(&mut h));
        assert_eq!(h.len(), 2);

        h.push(Message::user("failed turn"));
        assert!(repair_history_after_failure(&mut h));
        assert_eq!(h.len(), 2);
        assert_eq!(h.last().expect("history").role, "assistant");
    }

    #[test]
    fn repair_keeps_tool_results_to_avoid_orphaning_tool_use() {
        let mut h = vec![
            Message::user("go"),
            Message::assistant(vec![ContentBlock::ToolUse(ToolCall {
                id: "c1".into(),
                name: "bash".into(),
                input: serde_json::json!({}),
            })]),
            Message::tool_results(vec![ToolResult::ok("c1", "out")]),
        ];
        assert!(!repair_history_after_failure(&mut h));
        assert_eq!(h.len(), 3);
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        use async_trait::async_trait;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Flaky {
            failures_left: AtomicUsize,
            attempts: AtomicUsize,
        }

        #[async_trait]
        impl LlmProvider for Flaky {
            async fn complete(
                &self,
                _system: Option<&str>,
                _messages: &[Message],
                _tools: &[serde_json::Value],
            ) -> Result<Completion> {
                self.attempts.fetch_add(1, Ordering::SeqCst);
                if self.failures_left.fetch_sub(1, Ordering::SeqCst) > 0 {
                    return Err(anyhow::anyhow!("LLM returned 503: overloaded"));
                }
                Ok(Completion {
                    text: Some("recovered".into()),
                    tool_calls: Vec::new(),
                })
            }
        }

        let llm = Flaky {
            failures_left: AtomicUsize::new(2),
            attempts: AtomicUsize::new(0),
        };
        let reg = ToolRegistry::new();
        let mut history = Vec::new();
        let out = run(
            &llm,
            &reg,
            None,
            &mut history,
            "hi",
            RunLimits::iterations(3),
            &ToolContext::default(),
        )
        .await
        .expect("retry should recover");
        assert_eq!(out, "recovered");
        assert_eq!(llm.attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_gives_up_on_permanent_errors() {
        use async_trait::async_trait;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Broken {
            attempts: AtomicUsize,
        }

        #[async_trait]
        impl LlmProvider for Broken {
            async fn complete(
                &self,
                _system: Option<&str>,
                _messages: &[Message],
                _tools: &[serde_json::Value],
            ) -> Result<Completion> {
                self.attempts.fetch_add(1, Ordering::SeqCst);
                Err(anyhow::anyhow!("tool_use without id"))
            }
        }

        let llm = Broken {
            attempts: AtomicUsize::new(0),
        };
        let reg = ToolRegistry::new();
        let mut history = Vec::new();
        let err = run(
            &llm,
            &reg,
            None,
            &mut history,
            "hi",
            RunLimits::iterations(3),
            &ToolContext::default(),
        )
        .await
        .expect_err("permanent errors must not be retried");
        assert!(err.to_string().contains("tool_use without id"));
        assert_eq!(llm.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mixed_dispatch_preserves_input_order() {
        use async_trait::async_trait;

        struct Ro;
        #[async_trait]
        impl Tool for Ro {
            fn name(&self) -> &str {
                "ro"
            }
            fn description(&self) -> &str {
                "read-only"
            }
            fn schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            fn is_read_only(&self) -> bool {
                true
            }
            async fn execute(
                &self,
                input: serde_json::Value,
                _ctx: &ToolContext,
            ) -> Result<ToolOutput> {
                Ok(ToolOutput::ok(
                    input
                        .get("v")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                ))
            }
        }

        struct Mutating;
        #[async_trait]
        impl Tool for Mutating {
            fn name(&self) -> &str {
                "mut_tool"
            }
            fn description(&self) -> &str {
                "mutating"
            }
            fn schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            async fn execute(
                &self,
                input: serde_json::Value,
                _ctx: &ToolContext,
            ) -> Result<ToolOutput> {
                Ok(ToolOutput::ok(
                    input
                        .get("v")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                ))
            }
        }

        let mut reg = ToolRegistry::new();
        reg.insert_always(std::sync::Arc::new(Ro));
        reg.insert_always(std::sync::Arc::new(Mutating));
        let ctx = ToolContext::default();
        let calls = vec![
            ToolCall {
                id: "1".into(),
                name: "mut_tool".into(),
                input: serde_json::json!({"v": "first"}),
            },
            ToolCall {
                id: "2".into(),
                name: "ro".into(),
                input: serde_json::json!({"v": "second"}),
            },
            ToolCall {
                id: "3".into(),
                name: "mut_tool".into(),
                input: serde_json::json!({"v": "third"}),
            },
        ];
        let results = dispatch_tool_calls(&reg, &calls, &ctx).await;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].tool_use_id, "1");
        assert_eq!(results[0].content, "first");
        assert_eq!(results[1].tool_use_id, "2");
        assert_eq!(results[1].content, "second");
        assert_eq!(results[2].tool_use_id, "3");
        assert_eq!(results[2].content, "third");
    }

    #[tokio::test]
    async fn mutating_tools_run_serially() {
        use async_trait::async_trait;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Serial {
            live: std::sync::Arc<AtomicUsize>,
            max_live: std::sync::Arc<AtomicUsize>,
        }
        #[async_trait]
        impl Tool for Serial {
            fn name(&self) -> &str {
                "serial"
            }
            fn description(&self) -> &str {
                "mutating"
            }
            fn schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            async fn execute(
                &self,
                _input: serde_json::Value,
                _ctx: &ToolContext,
            ) -> Result<ToolOutput> {
                let n = self.live.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_live.fetch_max(n, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                self.live.fetch_sub(1, Ordering::SeqCst);
                Ok(ToolOutput::ok("ok"))
            }
        }

        let live = std::sync::Arc::new(AtomicUsize::new(0));
        let max_live = std::sync::Arc::new(AtomicUsize::new(0));
        let mut reg = ToolRegistry::new();
        reg.insert_always(std::sync::Arc::new(Serial {
            live: live.clone(),
            max_live: max_live.clone(),
        }));
        let ctx = ToolContext::default();
        let calls: Vec<ToolCall> = (0..4)
            .map(|i| ToolCall {
                id: format!("c{i}"),
                name: "serial".into(),
                input: serde_json::json!({}),
            })
            .collect();
        let results = dispatch_tool_calls(&reg, &calls, &ctx).await;
        assert_eq!(results.len(), 4);
        assert_eq!(
            max_live.load(Ordering::SeqCst),
            1,
            "mutating tools must not overlap"
        );
    }
}

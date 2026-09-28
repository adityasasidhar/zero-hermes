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
pub const DEFAULT_MAX_ITERATIONS: usize = 10;

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
        let mut completion = llm.complete(system, history, &tool_schemas).await?;
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

        let results = futures::future::join_all(
            completion
                .tool_calls
                .iter()
                .map(|tc| dispatch_tool(tools, tc, ctx)),
        )
        .await;
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
        // push to it below.
        let (completion, current_text) = {
            let mut stream = llm.stream(system, history, &tool_schemas);

            let mut current_text = String::new();
            let mut think_filter = ThinkFilter::default();
            let mut event: Option<StreamEvent> = None;

            while let Some(item) = stream.next().await {
                let ev = item?;
                match ev {
                    StreamEvent::TextDelta(s) => {
                        let visible = think_filter.push(&s);
                        if !visible.is_empty() {
                            on_event(StreamTurn::TextDelta(visible.clone()));
                            current_text.push_str(&visible);
                        }
                    }
                    StreamEvent::ToolUseBlock(tc) => {
                        on_event(StreamTurn::ToolUse(tc));
                    }
                    StreamEvent::Done(mut completion) => {
                        let visible = think_filter.finish();
                        if !visible.is_empty() {
                            on_event(StreamTurn::TextDelta(visible.clone()));
                            current_text.push_str(&visible);
                        }
                        completion.text = (!current_text.is_empty()).then(|| current_text.clone());
                        event = Some(StreamEvent::Done(completion));
                        break;
                    }
                }
            }
            let Some(StreamEvent::Done(completion)) = event else {
                anyhow::bail!("LLM stream ended without a Done event");
            };
            (completion, current_text)
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

        let results = futures::future::join_all(
            completion
                .tool_calls
                .iter()
                .map(|tc| dispatch_tool(tools, tc, ctx)),
        )
        .await;
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
}

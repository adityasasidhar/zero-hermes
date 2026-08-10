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
pub mod tool;

use std::sync::Arc;

pub use crate::agent::context::Context;
pub use crate::agent::provider::{build_provider, AnthropicMessages, Completion, LlmProvider, OpenAiCompat};
pub use crate::agent::tool::{
    ContentBlock, Message, Tool, ToolCall, ToolContext, ToolOutput, ToolResult,
};

use crate::error::Result;
use crate::tools::ToolRegistry;

/// Maximum iterations the outer loop will run before giving up.
pub const DEFAULT_MAX_ITERATIONS: usize = 10;

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
    max_iterations: usize,
    ctx: &ToolContext,
) -> Result<String> {
    history.push(Message::user(user));
    let tool_schemas = tools.schemas();
    let mut last_text: Option<String> = None;

    for iteration in 0..max_iterations {
        tracing::debug!(iteration, "agent loop iteration");
        let completion = llm.complete(system, history, &tool_schemas).await?;

        if completion.tool_calls.is_empty() {
            let text = completion.text.unwrap_or_default();
            history.push(Message::assistant_text(&text));
            return Ok(text);
        }

        let mut blocks: Vec<ContentBlock> = Vec::new();
        if let Some(text) = &completion.text {
            if !text.is_empty() {
                blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }
        for tc in &completion.tool_calls {
            blocks.push(ContentBlock::ToolUse(tc.clone()));
        }
        history.push(Message::assistant(blocks));
        last_text = completion.text;

        let mut results = Vec::with_capacity(completion.tool_calls.len());
        for tc in &completion.tool_calls {
            results.push(dispatch_tool(tools, tc, ctx).await);
        }
        history.push(Message::tool_results(results));
    }

    let fallback = last_text.unwrap_or_else(|| {
        "(agent loop hit max iterations without producing a final answer)".to_string()
    });
    history.push(Message::assistant_text(&fallback));
    Ok(fallback)
}

/// Dispatch a single tool call. Tool errors are converted to `is_error: true`
/// results so the model can recover.
pub async fn dispatch_tool(tools: &ToolRegistry, call: &ToolCall, ctx: &ToolContext) -> ToolResult {
    match tools.call(&call.name, call.input.clone(), ctx).await {
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

// Integration tests for the agent loop live in `tests/loop_test.rs`.

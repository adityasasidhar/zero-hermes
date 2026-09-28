//! Behavioural tests for `agent::run_stream`.
//!
//! `run_stream` is the loop behind `chat` and the web UI, and it is a
//! different code path from `run` (it accumulates text itself, filters
//! ` thinking` blocks, retries only *before* the first byte reaches the user,
//! and rebuilds the assistant turn from streamed tool calls). A bug here is
//! visible to the user as duplicated text or a silently dropped tool call.
//!
//! The provider below scripts one transcript per `stream()` call, so retry
//! and no-retry behaviour can be asserted by counting how many transcripts
//! were consumed.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::Stream;
use serde_json::{json, Value};

use zero_hermes::agent::stream::StreamEvent;
use zero_hermes::agent::tool::{Tool, ToolContext, ToolOutput};
use zero_hermes::agent::{
    make_context_with_memory, run_stream, Completion, LlmProvider, Message, RunLimits, StreamTurn,
    ToolCall,
};
use zero_hermes::memory::Memory;
use zero_hermes::tools::ToolRegistry;

type Item = zero_hermes::error::Result<StreamEvent>;

/// A provider that replays one scripted transcript per `stream()` call.
struct StreamScript {
    transcripts: Mutex<Vec<Vec<Item>>>,
    calls: Mutex<usize>,
}

impl StreamScript {
    fn new(transcripts: Vec<Vec<Item>>) -> Self {
        Self {
            transcripts: Mutex::new(transcripts),
            calls: Mutex::new(0),
        }
    }

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait]
impl LlmProvider for StreamScript {
    async fn complete(
        &self,
        _system: Option<&str>,
        _messages: &[Message],
        _tools: &[Value],
    ) -> zero_hermes::error::Result<Completion> {
        panic!("run_stream must call stream(), never complete()");
    }

    fn stream<'a>(
        &'a self,
        _system: Option<&'a str>,
        _messages: &'a [Message],
        _tools: &'a [Value],
    ) -> Pin<Box<dyn Stream<Item = Item> + Send + 'a>>
    where
        Self: 'a,
    {
        *self.calls.lock().unwrap() += 1;
        let mut transcripts = self.transcripts.lock().unwrap();
        let items = if transcripts.is_empty() {
            Vec::new()
        } else {
            transcripts.remove(0)
        };
        Box::pin(futures::stream::iter(items))
    }
}

fn text(s: &str) -> Item {
    Ok(StreamEvent::TextDelta(s.to_string()))
}

fn tool_use(id: &str, name: &str, input: Value) -> Item {
    Ok(StreamEvent::ToolUseBlock(ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        input,
    }))
}

fn done_text(s: &str) -> Item {
    Ok(StreamEvent::Done(Completion {
        text: Some(s.to_string()),
        tool_calls: Vec::new(),
    }))
}

fn done_with_calls(calls: Vec<ToolCall>) -> Item {
    Ok(StreamEvent::Done(Completion {
        text: None,
        tool_calls: calls,
    }))
}

fn stream_error(msg: &str) -> Item {
    Err(anyhow::anyhow!("{msg}"))
}

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "echo the `text` argument back"
    }
    fn schema(&self) -> Value {
        json!({"type": "object", "properties": {"text": {"type": "string"}}})
    }
    async fn execute(
        &self,
        input: Value,
        _ctx: &ToolContext,
    ) -> zero_hermes::error::Result<ToolOutput> {
        Ok(ToolOutput::ok(
            input
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ))
    }
}

fn echo_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(EchoTool));
    reg
}

fn visible_text(events: &[StreamTurn]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            StreamTurn::TextDelta(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

/// A streamed tool call must be dispatched, its result reported to the UI,
/// and its output fed back to the model — all in one `run_stream` call.
#[tokio::test]
async fn run_stream_dispatches_tool_calls_and_emits_results() {
    let call = ToolCall {
        id: "c1".to_string(),
        name: "echo".into(),
        input: json!({"text": "from-tool"}),
    };
    let provider = StreamScript::new(vec![
        vec![
            text("checking "),
            tool_use("c1", "echo", json!({"text": "from-tool"})),
            done_with_calls(vec![call]),
        ],
        vec![text("all done"), done_text("all done")],
    ]);
    let reg = echo_registry();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let out = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(5),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await
    .expect("run_stream");

    assert_eq!(out, "all done");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamTurn::ToolUse(tc) if tc.name == "echo")),
        "the UI should see the tool request: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            StreamTurn::ToolResult { name, output, is_error }
                if name == "echo" && output == "from-tool" && !is_error
        )),
        "the UI should see the tool result: {events:?}"
    );
    // The second model call must have been given the tool result.
    let saw_result = history.iter().flat_map(|m| &m.content).any(|b| {
        matches!(
            b,
            zero_hermes::agent::ContentBlock::ToolResult { content, .. } if content == "from-tool"
        )
    });
    assert!(saw_result, "the tool result never reached the model");
}

/// The streamed assistant turn must preserve text that accompanied a tool
/// call. Dropping it loses the model's stated intent from the transcript.
#[tokio::test]
async fn run_stream_keeps_assistant_text_alongside_a_tool_call() {
    let call = ToolCall {
        id: "c1".to_string(),
        name: "echo".into(),
        input: json!({"text": "x"}),
    };
    let provider = StreamScript::new(vec![
        vec![text("I will check"), done_with_calls(vec![call])],
        vec![text("done"), done_text("done")],
    ]);
    let reg = echo_registry();
    let mut history = Vec::new();
    let out = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(5),
        &ToolContext::default(),
        |_| {},
    )
    .await
    .expect("run_stream");
    assert_eq!(out, "done");

    let assistant_text = history
        .iter()
        .filter(|m| m.role == "assistant")
        .flat_map(|m| &m.content)
        .find_map(|b| match b {
            zero_hermes::agent::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        });
    assert_eq!(
        assistant_text.as_deref(),
        Some("I will check"),
        "assistant text before a tool call must survive into history"
    );
}

/// A model that only ever calls tools must terminate at `max_iterations`
/// with a readable fallback rather than looping forever.
#[tokio::test]
async fn run_stream_stops_at_the_iteration_cap_with_a_fallback() {
    let transcripts = (0..4)
        .map(|i| {
            let call = ToolCall {
                id: format!("c{i}"),
                name: "echo".into(),
                input: json!({"text": "x"}),
            };
            vec![done_with_calls(vec![call])]
        })
        .collect();
    let provider = StreamScript::new(transcripts);
    let reg = echo_registry();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let out = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "loop",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await
    .expect("the cap is a normal stop, not an error");

    assert!(
        out.contains("max iterations"),
        "the user should get an explanation, got: {out}"
    );
    assert!(matches!(events.last(), Some(StreamTurn::Done(_))));
    assert_eq!(
        provider.calls(),
        3,
        "the cap should stop at exactly 3 turns"
    );
}

// ---------------------------------------------------------------------------
// Think-block filtering
// ---------------------------------------------------------------------------

/// Reasoning blocks must never reach the user, including when the tags are
/// split across deltas — which is the normal case for a token stream.
#[tokio::test]
async fn run_stream_never_shows_a_think_block() {
    let provider = StreamScript::new(vec![vec![
        text("visible start <thi"),
        text("nk>false reasoning that must not leak"),
        text(" more hidden thought</thi"),
        text("nk>visible end"),
        done_text("ignored-by-the-loop"),
    ]]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let out = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await
    .expect("run_stream");

    let shown = visible_text(&events);
    assert!(
        !shown.contains("hidden thought") && !shown.contains("must not leak"),
        "reasoning leaked to the user: {shown:?}"
    );
    assert_eq!(
        shown, "visible start visible end",
        "only the text outside the think block should be shown"
    );
    assert_eq!(out, shown, "the returned answer should be what was shown");
}

/// The trait's default `stream()` (used by every provider without real SSE
/// support) emits a single `Done` and no deltas. That completion carries the
/// entire answer, so it must not be discarded.
#[tokio::test]
async fn run_stream_uses_a_completion_delivered_only_via_done() {
    struct CompletesOnly;

    #[async_trait]
    impl LlmProvider for CompletesOnly {
        async fn complete(
            &self,
            _system: Option<&str>,
            _messages: &[Message],
            _tools: &[Value],
        ) -> zero_hermes::error::Result<Completion> {
            Ok(Completion {
                text: Some("from-a-non-streaming-provider".into()),
                tool_calls: Vec::new(),
            })
        }
        // Deliberately no `stream()` override.
    }

    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let out = run_stream(
        &CompletesOnly,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await
    .expect("run_stream");

    assert_eq!(
        out, "from-a-non-streaming-provider",
        "a Done-only completion must not be dropped"
    );
    assert_eq!(
        visible_text(&events),
        "from-a-non-streaming-provider",
        "the UI must still receive the text as a delta"
    );
    assert!(
        history
            .iter()
            .any(|m| m.role == "assistant" && m.text().contains("non-streaming")),
        "the answer must be recorded in history"
    );
}

// ---------------------------------------------------------------------------
// Error handling and retry
// ---------------------------------------------------------------------------

/// A transient failure before anything has been shown is safe to retry, and
/// the user should never learn it happened.
#[tokio::test(start_paused = true)]
async fn run_stream_retries_a_transient_error_before_any_output() {
    let provider = StreamScript::new(vec![
        vec![stream_error("503 Service Unavailable: provider overloaded")],
        vec![text("recovered"), done_text("recovered")],
    ]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let out = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await
    .expect("the retry should have succeeded");

    assert_eq!(out, "recovered");
    assert_eq!(provider.calls(), 2, "one retry was expected");
    assert!(
        !visible_text(&events).contains("503"),
        "an internal retry must not surface to the user: {events:?}"
    );
}

/// Once a delta has been shown, retrying would duplicate it — so the loop
/// must give up instead. This is the guard that keeps the UI from printing
/// the same sentence twice.
#[tokio::test(start_paused = true)]
async fn run_stream_does_not_retry_after_output_was_already_emitted() {
    let provider = StreamScript::new(vec![
        vec![
            text("partial answer"),
            stream_error("503 temporary failure"),
        ],
        vec![text("DUPLICATE"), done_text("DUPLICATE")],
    ]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();

    let result = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |e| events.push(e),
    )
    .await;

    assert!(
        result.is_err(),
        "a mid-stream failure must be reported, got: {result:?}"
    );
    assert_eq!(
        provider.calls(),
        1,
        "a stream that already produced output must not be replayed"
    );
    assert!(
        !visible_text(&events).contains("DUPLICATE"),
        "the aborted turn must not be re-run: {events:?}"
    );
    assert!(
        visible_text(&events).contains("partial answer"),
        "text already shown to the user must not be retracted: {events:?}"
    );
}

/// A permanent error (bad key, bad request) is not worth retrying.
#[tokio::test(start_paused = true)]
async fn run_stream_does_not_retry_a_permanent_error() {
    let provider = StreamScript::new(vec![
        vec![stream_error("401 Unauthorized: invalid api key")],
        vec![done_text("SHOULD-NOT-RUN")],
    ]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();

    let result = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |_| {},
    )
    .await;

    assert!(result.is_err(), "a permanent error must surface");
    assert_eq!(provider.calls(), 1, "permanent errors must not be retried");
}

/// A stream that simply stops — no `Done`, no error — is a provider bug, but
/// it must be reported rather than silently treated as an empty answer.
#[tokio::test(start_paused = true)]
async fn run_stream_reports_a_stream_that_never_produced_done() {
    let provider = StreamScript::new(vec![vec![text("truncated"), text(" output")]]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();

    let err = run_stream(
        &provider,
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &ToolContext::default(),
        |_| {},
    )
    .await
    .expect_err("a missing Done event must not look like success");

    assert!(
        format!("{err:#}").contains("Done"),
        "the error should name the missing event: {err:#}"
    );
}

// ---------------------------------------------------------------------------
// Context construction
// ---------------------------------------------------------------------------

#[test]
fn make_context_with_memory_attaches_the_store_and_optional_cwd() {
    let mem = Arc::new(Memory::in_memory().unwrap());

    let ctx = make_context_with_memory(None, mem.clone());
    assert!(ctx.memory.is_some(), "the memory handle must be wired up");
    assert!(ctx.cwd.is_none());
    assert!(ctx.session_id.is_none());

    let ctx = make_context_with_memory(Some(std::path::PathBuf::from("/tmp/somewhere")), mem);
    assert_eq!(
        ctx.cwd.as_deref(),
        Some(std::path::Path::new("/tmp/somewhere"))
    );
    assert!(ctx.memory.is_some());
}

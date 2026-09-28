//! Integration tests for the agent loop.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use zero_hermes::agent::{Completion, LlmProvider, Message, RunLimits, Tool};
use zero_hermes::tools::ToolRegistry;

#[derive(Clone)]
struct ScriptedProvider {
    steps: Arc<Mutex<Vec<Completion>>>,
    calls: Arc<Mutex<Vec<Vec<Message>>>>,
}

impl ScriptedProvider {
    fn new(steps: Vec<Completion>) -> Self {
        Self {
            steps: Arc::new(Mutex::new(steps)),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    async fn complete(
        &self,
        _system: Option<&str>,
        messages: &[Message],
        _tools: &[Value],
    ) -> zero_hermes::error::Result<Completion> {
        self.calls.lock().unwrap().push(messages.to_vec());
        let mut s = self.steps.lock().unwrap();
        Ok(if s.is_empty() {
            Completion {
                text: Some("(end)".into()),
                tool_calls: Vec::new(),
            }
        } else {
            s.remove(0)
        })
    }
}

struct Echo;

#[async_trait]
impl Tool for Echo {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "echo"
    }
    fn schema(&self) -> Value {
        json!({"type": "object", "properties": {"text": {"type": "string"}}})
    }
    async fn execute(
        &self,
        input: Value,
        _ctx: &zero_hermes::agent::ToolContext,
    ) -> zero_hermes::error::Result<zero_hermes::agent::ToolOutput> {
        let s = input
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(zero_hermes::agent::ToolOutput::ok(s))
    }
}

#[tokio::test]
async fn loop_dispatch_single_tool() {
    let provider = ScriptedProvider::new(vec![
        Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: "1".into(),
                name: "echo".into(),
                input: json!({"text": "ping"}),
            }],
        },
        Completion {
            text: Some("got: ping".into()),
            tool_calls: Vec::new(),
        },
    ]);
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(Echo));
    let mut history = Vec::new();
    let out = zero_hermes::agent::run(
        &provider,
        &reg,
        Some("sys"),
        &mut history,
        "do it",
        RunLimits::iterations(5),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(out, "got: ping");
    assert_eq!(history.len(), 4);
}

#[tokio::test]
async fn loop_handles_unknown_tool() {
    let provider = ScriptedProvider::new(vec![
        Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: "1".into(),
                name: "missing".into(),
                input: json!({}),
            }],
        },
        Completion {
            text: Some("recovered".into()),
            tool_calls: Vec::new(),
        },
    ]);
    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let out = zero_hermes::agent::run(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(5),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(out, "recovered");
}

#[tokio::test]
async fn loop_caps_iterations_to_max() {
    let steps = (0..5)
        .map(|_| Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: "x".into(),
                name: "echo".into(),
                input: json!({"text": "loop"}),
            }],
        })
        .collect();
    let provider = ScriptedProvider::new(steps);
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(Echo));
    let mut history = Vec::new();
    let out = zero_hermes::agent::run(
        &provider,
        &reg,
        None,
        &mut history,
        "loop forever",
        RunLimits::iterations(3),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert!(out.contains("max iterations"));
}

#[tokio::test]
async fn run_stream_with_mock_yields_text_then_done() {
    use std::sync::Arc;
    use zero_hermes::agent::mock::MockProvider;
    use zero_hermes::agent::{run_stream, StreamTurn};

    let provider = Arc::new(MockProvider::text_only("hello-stream"));
    let reg = ToolRegistry::new();
    let mut history = Vec::new();
    let mut events: Vec<StreamTurn> = Vec::new();
    let out = run_stream(
        provider.as_ref(),
        &reg,
        None,
        &mut history,
        "hi",
        RunLimits::iterations(3),
        &zero_hermes::agent::ToolContext::default(),
        |ev| events.push(ev),
    )
    .await
    .unwrap();
    assert_eq!(out, "hello-stream");
    // Should see at least one TextDelta and one Done.
    assert!(events.iter().any(|e| matches!(e, StreamTurn::TextDelta(_))));
    assert!(events.iter().any(|e| matches!(e, StreamTurn::Done(_))));
}

#[tokio::test]
async fn loop_uses_tool_result_in_next_call() {
    let provider = ScriptedProvider::new(vec![
        Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: "1".into(),
                name: "echo".into(),
                input: json!({"text": "hello"}),
            }],
        },
        Completion {
            text: Some("done".into()),
            tool_calls: Vec::new(),
        },
    ]);
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(Echo));
    let mut history = Vec::new();
    zero_hermes::agent::run(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(5),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    let calls = provider.calls.lock().unwrap();
    let second = &calls[1];
    let mut saw_result = false;
    for m in second {
        for b in &m.content {
            if let zero_hermes::agent::ContentBlock::ToolResult { content, .. } = b {
                if content.contains("hello") {
                    saw_result = true;
                }
            }
        }
    }
    assert!(saw_result);
}

/// A tool that always reports failure the way the builtins do: `Ok` with
/// `is_error` set, not `Err`.
struct FailingTool;

#[async_trait::async_trait]
impl Tool for FailingTool {
    fn name(&self) -> &str {
        "boom"
    }
    fn description(&self) -> &str {
        "always fails"
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &zero_hermes::agent::ToolContext,
    ) -> zero_hermes::error::Result<zero_hermes::agent::ToolOutput> {
        Ok(zero_hermes::agent::ToolOutput::err("exit code 1"))
    }
}

#[tokio::test]
async fn tool_failures_reach_the_model_flagged_as_errors() {
    use zero_hermes::agent::ContentBlock;

    let provider = ScriptedProvider::new(vec![
        Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: "c1".into(),
                name: "boom".into(),
                input: serde_json::json!({}),
            }],
        },
        Completion {
            text: Some("recovered".into()),
            tool_calls: Vec::new(),
        },
    ]);
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(FailingTool));
    let mut history = Vec::new();
    let out = zero_hermes::agent::run(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(5),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(out, "recovered");

    // The builtins signal failure via `Ok(ToolOutput::err(..))`; dropping
    // the flag told the model every failed command had succeeded.
    let result_block = history
        .iter()
        .flat_map(|m| &m.content)
        .find_map(|b| match b {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .expect("a tool_result block");
    assert_eq!(result_block.0, "exit code 1");
    assert!(result_block.1, "is_error must survive dispatch");
}

#[tokio::test]
async fn history_stays_bounded_across_many_turns() {
    // The runaway case is a long-lived gateway chat: `run` is called over
    // and over against the same `history`, which only ever grew. Ten turns
    // with a six-message window must stay bounded and well-formed.
    use zero_hermes::agent::ContentBlock;

    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(Echo));
    let mut history = Vec::new();

    for turn in 0..10 {
        let provider = ScriptedProvider::new(vec![
            Completion {
                text: None,
                tool_calls: vec![zero_hermes::agent::ToolCall {
                    id: format!("c{turn}"),
                    name: "echo".into(),
                    input: serde_json::json!({"text": "x"}),
                }],
            },
            Completion {
                text: Some(format!("answer {turn}")),
                tool_calls: Vec::new(),
            },
        ]);
        zero_hermes::agent::run(
            &provider,
            &reg,
            None,
            &mut history,
            &format!("question {turn}"),
            RunLimits::iterations(5).with_context_window(6),
            &zero_hermes::agent::ToolContext::default(),
        )
        .await
        .unwrap();
    }

    // Each turn appends 4 messages; without compaction this would be 40.
    assert!(
        history.len() <= 10,
        "history should stay bounded, got {}",
        history.len()
    );
    assert_eq!(history[0].role, "user");
    for (i, m) in history.iter().enumerate() {
        let has_result = m
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
        if has_result {
            assert!(
                i > 0
                    && history[i - 1]
                        .content
                        .iter()
                        .any(|b| matches!(b, ContentBlock::ToolUse(_))),
                "orphaned tool_result at {i}"
            );
        }
    }
}

#[tokio::test]
async fn a_single_turn_is_never_cut_mid_tool_chain() {
    // Within one turn there is no second user message to cut at, so
    // compaction correctly declines rather than orphaning a tool_result.
    // Growth here is already bounded by `max_iterations`.
    let mut script: Vec<Completion> = (0..4)
        .map(|i| Completion {
            text: None,
            tool_calls: vec![zero_hermes::agent::ToolCall {
                id: format!("c{i}"),
                name: "echo".into(),
                input: serde_json::json!({"text": "x"}),
            }],
        })
        .collect();
    script.push(Completion {
        text: Some("done".into()),
        tool_calls: Vec::new(),
    });

    let provider = ScriptedProvider::new(script);
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(Echo));
    let mut history = Vec::new();
    let out = zero_hermes::agent::run(
        &provider,
        &reg,
        None,
        &mut history,
        "go",
        RunLimits::iterations(10).with_context_window(2),
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(out, "done");
    assert_eq!(
        history[0].role, "user",
        "the opening user turn is never dropped"
    );
    assert_eq!(history.len(), 10, "1 user + 4 tool round-trips + 1 answer");
}

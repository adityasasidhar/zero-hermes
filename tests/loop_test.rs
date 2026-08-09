//! Integration tests for the agent loop.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

use zero_hermes::agent::{Completion, LlmProvider, Message, Tool};
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
        5,
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
        5,
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
        3,
        &zero_hermes::agent::ToolContext::default(),
    )
    .await
    .unwrap();
    assert!(out.contains("max iterations"));
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
        5,
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

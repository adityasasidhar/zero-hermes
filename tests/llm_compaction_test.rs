//! Opt-in model-written compaction inside the agent loop.

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;
use zero_hermes::agent::tool::ToolContext;
use zero_hermes::agent::{run, Completion, LlmProvider, Message, RunLimits};
use zero_hermes::tools::ToolRegistry;

/// Answers the summariser call with a marker and everything else with "done",
/// recording what the main call saw.
#[derive(Default)]
struct Spy {
    summarizer_calls: Mutex<usize>,
    main_first_message: Mutex<Option<String>>,
}

#[async_trait]
impl LlmProvider for Spy {
    async fn complete(
        &self,
        system: Option<&str>,
        messages: &[Message],
        _tools: &[Value],
    ) -> zero_hermes::error::Result<Completion> {
        let text = if system.is_some_and(|s| s.contains("compress conversations")) {
            if let Ok(mut n) = self.summarizer_calls.lock() {
                *n += 1;
            }
            "LOOP-SUMMARY"
        } else {
            if let (Ok(mut slot), Some(first)) = (self.main_first_message.lock(), messages.first())
            {
                *slot = Some(first.text());
            }
            "done"
        };
        Ok(Completion {
            text: Some(text.to_string()),
            tool_calls: Vec::new(),
        })
    }
}

fn big_history() -> Vec<Message> {
    let mut h = Vec::new();
    for i in 0..30 {
        h.push(Message::user(format!("question {i} {}", "x".repeat(300))));
        h.push(Message::assistant_text(format!(
            "answer {i} {}",
            "y".repeat(300)
        )));
    }
    h
}

fn limits(llm: bool) -> RunLimits {
    RunLimits {
        context_tokens: 1_500,
        llm_compaction: llm,
        ..RunLimits::default()
    }
}

#[tokio::test]
async fn over_budget_history_is_summarised_by_the_model_when_enabled() {
    let spy = Spy::default();
    let mut history = big_history();
    let out = run(
        &spy,
        &ToolRegistry::new(),
        None,
        &mut history,
        "next question",
        limits(true),
        &ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(out, "done");
    assert_eq!(*spy.summarizer_calls.lock().unwrap(), 1);
    let first = spy.main_first_message.lock().unwrap().clone().unwrap();
    assert!(first.contains("LOOP-SUMMARY"), "{first}");
    assert!(first.contains("written by the model"), "{first}");
}

#[tokio::test]
async fn disabled_by_default_so_no_extra_provider_call_is_made() {
    let spy = Spy::default();
    let mut history = big_history();
    run(
        &spy,
        &ToolRegistry::new(),
        None,
        &mut history,
        "next question",
        limits(false),
        &ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(*spy.summarizer_calls.lock().unwrap(), 0);
    let first = spy.main_first_message.lock().unwrap().clone().unwrap();
    assert!(first.contains("extractive"), "{first}");
}

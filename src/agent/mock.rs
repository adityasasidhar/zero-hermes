//! Mock provider for tests and `--mock` runs.

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::pin::Pin;

use crate::agent::stream::StreamEvent;
use crate::agent::{Completion, LlmProvider, Message};
use crate::error::Result;

/// Provider that returns a canned response or an echo.
pub struct MockProvider {
    mode: MockMode,
}

enum MockMode {
    Text(String),
    Echo,
}

impl MockProvider {
    /// Return the given text for the first prompt.
    pub fn text_only(s: impl Into<String>) -> Self {
        Self {
            mode: MockMode::Text(s.into()),
        }
    }

    /// Always return a canned echo response.
    pub fn echo() -> Self {
        Self {
            mode: MockMode::Echo,
        }
    }

    fn make_completion(&self) -> Completion {
        match &self.mode {
            MockMode::Text(s) => Completion {
                text: Some(s.clone()),
                tool_calls: Vec::new(),
            },
            MockMode::Echo => Completion {
                text: Some("(mock) ok".to_string()),
                tool_calls: Vec::new(),
            },
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(
        &self,
        _system: Option<&str>,
        _messages: &[Message],
        _tools: &[Value],
    ) -> Result<Completion> {
        Ok(self.make_completion())
    }

    fn stream<'a>(
        &'a self,
        _system: Option<&'a str>,
        _messages: &'a [Message],
        _tools: &'a [Value],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + 'a>>
    where
        Self: 'a,
    {
        // Emit the full text in a single TextDelta followed by a Done.
        // The brief asks for one chunk, which is the simplest correct
        // behaviour for a mock and what real servers also produce when
        // a model returns a tiny final answer.
        let completion = self.make_completion();
        let text = completion.text.clone().unwrap_or_default();
        let mut events: Vec<Result<StreamEvent>> = Vec::new();
        if !text.is_empty() {
            events.push(Ok(StreamEvent::TextDelta(text.clone())));
        }
        events.push(Ok(StreamEvent::Done(Completion {
            text: Some(text),
            tool_calls: Vec::new(),
        })));
        Box::pin(futures::stream::iter(events))
    }
}

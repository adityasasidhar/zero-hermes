//! Mock provider for tests and `--mock` runs.

use async_trait::async_trait;
use serde_json::Value;

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
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(
        &self,
        _system: Option<&str>,
        _messages: &[Message],
        _tools: &[Value],
    ) -> Result<Completion> {
        match &self.mode {
            MockMode::Text(s) => Ok(Completion {
                text: Some(s.clone()),
                tool_calls: Vec::new(),
            }),
            MockMode::Echo => Ok(Completion {
                text: Some("(mock) ok".to_string()),
                tool_calls: Vec::new(),
            }),
        }
    }
}

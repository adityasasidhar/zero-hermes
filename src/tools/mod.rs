//! Tool registry: a small name -> tool map used by the agent loop.

pub mod builtin;

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::error::Result;

/// Map of tool name -> tool implementation.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. If `enabled` is non-empty, the tool is only inserted
    /// when its name appears in the list. If `enabled` is empty, all tools
    /// are inserted.
    pub fn insert(&mut self, tool: Arc<dyn Tool>, enabled: &[String]) {
        if enabled.is_empty() || enabled.iter().any(|n| n == tool.name()) {
            self.tools.insert(tool.name().to_string(), tool);
        }
    }

    /// Register a tool unconditionally.
    pub fn insert_always(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Remove a tool by name, returning the previous `Arc` if present.
    /// Used by [`SubAgentTool`] to strip itself out of the child registry
    /// without iterating all tools.
    pub fn remove(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.remove(name)
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Names of all registered tools.
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tools.keys().cloned().collect();
        v.sort();
        v
    }

    /// Iterate registered tools.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<dyn Tool>)> {
        self.tools.iter()
    }

    /// Build the JSON schemas to send to the LLM (one entry per tool).
    pub fn schemas(&self) -> Vec<Value> {
        let mut v: Vec<Value> = self
            .tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.schema(),
                })
            })
            .collect();
        v.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        v
    }

    /// Invoke a tool by name with the given input.
    pub async fn call(&self, name: &str, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown tool: {name}"))?;
        tool.execute(input, ctx).await
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("names", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct HelloTool;

    #[async_trait]
    impl Tool for HelloTool {
        fn name(&self) -> &str {
            "hello"
        }
        fn description(&self) -> &str {
            "say hello"
        }
        fn schema(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::ok("hi"))
        }
    }

    struct OtherTool;

    #[async_trait]
    impl Tool for OtherTool {
        fn name(&self) -> &str {
            "other"
        }
        fn description(&self) -> &str {
            "other"
        }
        fn schema(&self) -> Value {
            json!({"type": "object"})
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::ok("o"))
        }
    }

    #[tokio::test]
    async fn registry_lookup_and_call() {
        let mut reg = ToolRegistry::new();
        reg.insert_always(Arc::new(HelloTool));
        reg.insert_always(Arc::new(OtherTool));
        let out = reg
            .call("hello", json!({}), &ToolContext::default())
            .await
            .unwrap();
        assert_eq!(out.content, "hi");
    }

    #[test]
    fn registry_filter() {
        let mut reg = ToolRegistry::new();
        reg.insert(Arc::new(HelloTool), &["hello".into()]);
        reg.insert(Arc::new(OtherTool), &["hello".into()]);
        assert_eq!(reg.names(), vec!["hello".to_string()]);
    }

    #[test]
    fn registry_schemas_are_sorted() {
        let mut reg = ToolRegistry::new();
        reg.insert_always(Arc::new(OtherTool));
        reg.insert_always(Arc::new(HelloTool));
        let schemas = reg.schemas();
        let names: Vec<&str> = schemas
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["hello", "other"]);
    }

    #[tokio::test]
    async fn registry_unknown_tool() {
        let reg = ToolRegistry::new();
        let err = reg
            .call("nope", json!({}), &ToolContext::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[test]
    fn registry_remove_returns_arc() {
        let mut reg = ToolRegistry::new();
        reg.insert_always(Arc::new(HelloTool));
        assert!(reg.remove("hello").is_some());
        assert!(reg.remove("hello").is_none());
        assert!(reg.names().is_empty());
    }
}

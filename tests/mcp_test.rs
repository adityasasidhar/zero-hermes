//! MCP wiring: a fake stdio server (a `/bin/sh` script) is discovered at
//! startup and its tools are registered, callable and inherited by sub-agents.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use zero_hermes::agent::mock::MockProvider;
use zero_hermes::agent::provider::LlmProvider;
use zero_hermes::agent::tool::ToolContext;
use zero_hermes::bootstrap::{build_tool_registry, build_tool_registry_async};
use zero_hermes::config::{Config, McpServerConfig};

/// One request line in, one canned response line out, per process.
const FAKE_SERVER: &str = r#"
read line
case "$line" in
  *'"tools/list"'*) echo '{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"echo-it","description":"echo back","inputSchema":{"type":"object","properties":{"text":{"type":"string"}}}}]}}' ;;
  *'"tools/call"'*) echo '{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"pong"}]}}' ;;
  *) echo '{"jsonrpc":"2.0","id":1,"result":{}}' ;;
esac
"#;

fn cfg_with_fake_server() -> Config {
    let mut cfg = Config::default();
    cfg.mcp.servers.insert(
        "fake".to_string(),
        McpServerConfig {
            command: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), FAKE_SERVER.to_string()],
            env: HashMap::new(),
        },
    );
    // A broken server must not take the registry down.
    cfg.mcp.servers.insert(
        "broken".to_string(),
        McpServerConfig {
            command: "/nonexistent/binary".to_string(),
            args: vec![],
            env: HashMap::new(),
        },
    );
    cfg
}

fn provider() -> Arc<dyn LlmProvider> {
    Arc::new(MockProvider::text_only("ok"))
}

#[tokio::test]
async fn async_registry_discovers_and_calls_mcp_tools() {
    let cfg = cfg_with_fake_server();
    let reg = build_tool_registry_async(&cfg, provider()).await;
    assert!(reg.names().contains(&"mcp_fake_echo_it".to_string()));
    assert!(!reg.names().iter().any(|n| n.starts_with("mcp_broken")));
    let out = reg
        .call(
            "mcp_fake_echo_it",
            json!({"text": "ping"}),
            &ToolContext::default(),
        )
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);
    assert!(out.content.contains("pong"), "{}", out.content);
}

#[tokio::test]
async fn sync_registry_does_not_spawn_servers() {
    let reg = build_tool_registry(&cfg_with_fake_server(), provider());
    assert!(!reg.names().iter().any(|n| n.starts_with("mcp_")));
}

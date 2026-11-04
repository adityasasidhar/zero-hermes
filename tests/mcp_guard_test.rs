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

/// A persistent line-oriented MCP server. Replies carry the request id; the
/// `tools/call` result carries this process's pid so tests can tell whether
/// the session was reused or respawned. If `$MARKER` exists the server removes
/// it and exits without replying, simulating a server that died while idle.
const FAKE_SERVER: &str = r#"
while read line; do
  case "$line" in *'"id":'*) ;; *) continue ;; esac
  id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
  if [ -n "$MARKER" ] && [ -e "$MARKER" ]; then rm -f "$MARKER"; exit 0; fi
  echo 'log noise that is not json'
  case "$line" in
    *'"tools/list"'*) echo '{"jsonrpc":"2.0","id":'$id',"result":{"tools":[{"name":"echo-it","description":"echo back","inputSchema":{"type":"object","properties":{"text":{"type":"string"}}}}]}}' ;;
    *'"tools/call"'*) echo '{"jsonrpc":"2.0","method":"notifications/progress","params":{}}'; echo '{"jsonrpc":"2.0","id":'$id',"result":{"content":[{"type":"text","text":"pid='$$'"}]}}' ;;
    *) echo '{"jsonrpc":"2.0","id":'$id',"result":{}}' ;;
  esac
done
"#;

fn cfg_with_fake_server() -> Config {
    cfg_with_marker(None)
}

fn cfg_with_marker(marker: Option<&std::path::Path>) -> Config {
    let mut cfg = Config::default();
    let mut env = HashMap::new();
    if let Some(m) = marker {
        env.insert("MARKER".to_string(), m.display().to_string());
    }
    cfg.mcp.servers.insert(
        "fake".to_string(),
        McpServerConfig {
            command: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), FAKE_SERVER.to_string()],
            env,
            ..Default::default()
        },
    );
    // A broken server must not take the registry down.
    cfg.mcp.servers.insert(
        "broken".to_string(),
        McpServerConfig {
            command: "/nonexistent/binary".to_string(),
            args: vec![],
            env: HashMap::new(),
            ..Default::default()
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
    assert!(out.content.contains("pid="), "{}", out.content);
}

fn pid_of(out: &zero_hermes::agent::tool::ToolOutput) -> String {
    out.content
        .split("pid=")
        .nth(1)
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[tokio::test]
async fn session_is_reused_across_calls_and_respawns_after_death() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("die");
    let reg = build_tool_registry_async(&cfg_with_marker(Some(&marker)), provider()).await;
    let ctx = ToolContext::default();
    let call = || reg.call("mcp_fake_echo_it", json!({}), &ctx);

    let first = call().await.unwrap();
    let second = call().await.unwrap();
    assert!(!first.is_error && !second.is_error, "{first:?} {second:?}");
    assert_eq!(pid_of(&first), pid_of(&second), "process must be reused");

    // The server dies while idle: the next call heals on a fresh process.
    std::fs::write(&marker, "x").unwrap();
    let third = call().await.unwrap();
    assert!(!third.is_error, "{}", third.content);
    assert_ne!(pid_of(&third), pid_of(&first), "process must be respawned");
}

#[tokio::test]
async fn sync_registry_does_not_spawn_servers() {
    let reg = build_tool_registry(&cfg_with_fake_server(), provider());
    assert!(!reg.names().iter().any(|n| n.starts_with("mcp_")));
}

#[tokio::test]
async fn shell_tools_refuse_catastrophic_commands_unless_guard_is_off() {
    let ctx = ToolContext::default();
    let guarded = build_tool_registry(&Config::default(), provider());
    for tool in ["bash", "execute_command", "execute_code"] {
        let out = guarded
            .call(tool, json!({"command": "rm -rf /"}), &ctx)
            .await
            .unwrap();
        assert!(out.is_error, "{tool}: {}", out.content);
        assert!(out.content.contains("command guard"), "{}", out.content);
    }
    let ok = guarded
        .call("bash", json!({"command": "echo fine"}), &ctx)
        .await
        .unwrap();
    assert!(
        !ok.is_error && ok.content.contains("fine"),
        "{}",
        ok.content
    );

    let mut cfg = Config::default();
    cfg.agent.command_guard = false;
    let open = build_tool_registry(&cfg, provider());
    let out = open
        .call("bash", json!({"command": "echo shutdown -h"}), &ctx)
        .await
        .unwrap();
    assert!(!out.is_error, "{}", out.content);
}

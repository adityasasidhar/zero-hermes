//! MCP over streamable HTTP, against a tiny in-process mock server.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zero_hermes::agent::mock::MockProvider;
use zero_hermes::agent::tool::ToolContext;
use zero_hermes::bootstrap::build_tool_registry_async;
use zero_hermes::config::{Config, McpServerConfig};

/// Serves MCP. `tools/list` replies as plain JSON, `tools/call` as SSE.
/// Records how many `initialize` calls arrived and the auth header seen.
async fn serve(inits: Arc<AtomicUsize>, auth_ok: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (inits, auth_ok) = (inits.clone(), auth_ok.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let (head, body) = loop {
                    let n = sock.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let head = text[..i].to_lowercase();
                        let len = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if text.len() >= i + 4 + len {
                            break (head, text[i + 4..i + 4 + len].to_string());
                        }
                    }
                };
                if head.contains("authorization: bearer s3cret") {
                    auth_ok.fetch_add(1, Ordering::SeqCst);
                }
                let req: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let id = req.get("id").cloned().unwrap_or(Value::Null);
                let method = req["method"].as_str().unwrap_or("");
                let (ctype, extra, payload) = match method {
                    "initialize" => {
                        inits.fetch_add(1, Ordering::SeqCst);
                        (
                            "application/json",
                            "mcp-session-id: sess-1\r\n",
                            json!({"jsonrpc":"2.0","id":id,"result":{}}).to_string(),
                        )
                    }
                    "tools/list" => (
                        "application/json",
                        "",
                        json!({"jsonrpc":"2.0","id":id,"result":{"tools":[
                            {"name":"hello","description":"say hi","inputSchema":{"type":"object"}}
                        ]}})
                        .to_string(),
                    ),
                    "tools/call" => {
                        let sid_ok = head.contains("mcp-session-id: sess-1");
                        let text = if sid_ok { "hi-over-sse" } else { "NO-SESSION" };
                        let msg = json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}});
                        (
                            "text/event-stream",
                            "",
                            format!("event: message\ndata: {msg}\n\n"),
                        )
                    }
                    _ => ("application/json", "", String::new()),
                };
                let status = if method.starts_with("notifications/") {
                    "202 Accepted"
                } else {
                    "200 OK"
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: {ctype}\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    port
}

#[tokio::test]
async fn http_server_discovered_called_over_sse_with_session_and_headers() {
    let inits = Arc::new(AtomicUsize::new(0));
    let auth_ok = Arc::new(AtomicUsize::new(0));
    let port = serve(inits.clone(), auth_ok.clone()).await;

    let mut cfg = Config::default();
    cfg.mcp.servers.insert(
        "remote".to_string(),
        McpServerConfig {
            url: Some(format!("http://127.0.0.1:{port}/mcp")),
            headers: HashMap::from([("Authorization".to_string(), "Bearer s3cret".to_string())]),
            ..Default::default()
        },
    );
    let reg = build_tool_registry_async(&cfg, Arc::new(MockProvider::text_only("ok"))).await;
    assert!(reg.names().contains(&"mcp_remote_hello".to_string()));

    let ctx = ToolContext::default();
    for _ in 0..2 {
        let out = reg.call("mcp_remote_hello", json!({}), &ctx).await.unwrap();
        assert!(!out.is_error, "{}", out.content);
        assert!(out.content.contains("hi-over-sse"), "{}", out.content);
    }
    assert_eq!(inits.load(Ordering::SeqCst), 1, "handshake happens once");
    assert!(auth_ok.load(Ordering::SeqCst) >= 4, "custom headers sent");
}

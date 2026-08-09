//! Integration tests for the tool registry and builtin tools.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use zero_hermes::agent::tool::{Tool, ToolContext, ToolOutput};
use zero_hermes::tools::builtin::{BashTool, FetchTool, MemoryTool, ReadTool, WriteTool};
use zero_hermes::tools::ToolRegistry;

struct StrTool(&'static str, &'static str);

#[async_trait]
impl Tool for StrTool {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        self.1
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object"})
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> zero_hermes::error::Result<ToolOutput> {
        Ok(ToolOutput::ok(self.1))
    }
}

#[tokio::test]
async fn registry_lookup_and_call() {
    let mut reg = ToolRegistry::new();
    reg.insert_always(Arc::new(StrTool("hello", "greeting")));
    reg.insert_always(Arc::new(StrTool("bye", "farewell")));
    let out = reg
        .call("hello", json!({}), &ToolContext::default())
        .await
        .unwrap();
    assert_eq!(out.content, "greeting");
}

#[tokio::test]
async fn registry_filter_by_name() {
    let mut reg = ToolRegistry::new();
    reg.insert(Arc::new(StrTool("one", "1")), &["two".into()]);
    reg.insert(Arc::new(StrTool("two", "2")), &["two".into()]);
    assert_eq!(reg.names(), vec!["two"]);
}

#[tokio::test]
async fn bash_tool_runs_command() {
    let tool = BashTool;
    let out = tool
        .execute(
            json!({"command": "echo hello-world", "timeout": 5}),
            &ToolContext::default(),
        )
        .await
        .unwrap();
    assert!(out.content.contains("hello-world"));
}

#[tokio::test]
async fn read_and_write_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ToolContext {
        cwd: Some(tmp.path().to_path_buf()),
        ..Default::default()
    };
    let w = WriteTool;
    w.execute(json!({"path": "out.txt", "content": "abc\n"}), &ctx)
        .await
        .unwrap();
    let r = ReadTool;
    let out = r.execute(json!({"path": "out.txt"}), &ctx).await.unwrap();
    assert_eq!(out.content, "abc\n");
    let out = r
        .execute(json!({"path": "out.txt", "max_bytes": 2}), &ctx)
        .await
        .unwrap();
    assert_eq!(out.content, "ab");
}

#[tokio::test]
async fn fetch_tool_against_local_server() {
    // Spin up a tiny HTTP server with tokio's TcpListener.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let body = "{\"hello\":\"world\"}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes()).await;
        }
    });

    let tool = FetchTool::default();
    let url = format!("http://{addr}/json");
    let out = tool
        .execute(json!({"url": url}), &ToolContext::default())
        .await
        .unwrap();
    assert!(out.content.contains("hello"));
    let _ = task.await;
}

#[tokio::test]
async fn memory_tool_round_trip() {
    let mem = Arc::new(zero_hermes::memory::Memory::in_memory().unwrap());
    let ctx = ToolContext {
        memory: Some(mem.clone()),
        ..Default::default()
    };
    let tool = MemoryTool;
    tool.execute(json!({"action": "write", "key": "k", "value": "v"}), &ctx)
        .await
        .unwrap();
    let out = tool
        .execute(json!({"action": "read", "key": "k"}), &ctx)
        .await
        .unwrap();
    assert_eq!(out.content, "v");
    let out = tool.execute(json!({"action": "list"}), &ctx).await.unwrap();
    assert!(out.content.contains("k: v"));
    let out = tool
        .execute(json!({"action": "delete", "key": "k"}), &ctx)
        .await
        .unwrap();
    assert_eq!(out.content, "ok");
}

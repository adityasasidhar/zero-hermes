//! Built-in tools: `bash`, `read`, `write`, `fetch`, `memory`.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::error::Result;

/// Run a shell command and return stdout + stderr.
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn description(&self) -> &str {
        "Run a shell command and return combined stdout+stderr."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to run"},
                "timeout": {"type": "integer", "description": "Timeout in seconds (default 10)"}
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            command: String,
            timeout: Option<u64>,
        }
        let args: Args = serde_json::from_value(input)?;
        let timeout = Duration::from_secs(args.timeout.unwrap_or(10));

        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(&args.command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null());
        if let Some(cwd) = &ctx.cwd {
            cmd.current_dir(cwd);
        }
        let result = tokio::task::spawn_blocking(move || run_with_timeout(cmd, timeout)).await;
        match result {
            Ok(Ok(out)) => Ok(ToolOutput::ok(out)),
            Ok(Err(e)) => Ok(ToolOutput::err(format!("bash failed: {e}"))),
            Err(e) => Ok(ToolOutput::err(format!("bash join error: {e}"))),
        }
    }
}

/// Run a command with a timeout. If the timeout expires, the child is killed.
fn run_with_timeout(mut cmd: std::process::Command, timeout: Duration) -> std::io::Result<String> {
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread;

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let t_out = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = out_pipe.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let t_err = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = err_pipe.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let wait_thread = thread::spawn(move || {
        let r = child.wait();
        let _ = tx.send(r);
    });

    let status = match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(_) => {
            kill_process(pid);
            let _ = rx.recv_timeout(Duration::from_secs(2));
            Ok(synthetic_status())
        }
    };
    let _ = wait_thread.join();

    let out_buf = t_out.join().unwrap_or_default();
    let err_buf = t_err.join().unwrap_or_default();
    let mut out = String::from_utf8_lossy(&out_buf).to_string();
    let err = String::from_utf8_lossy(&err_buf);
    if !err.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&err);
    }
    if let Ok(s) = status {
        if let Some(code) = s.code() {
            if code != 0 {
                out.push_str(&format!("\n[exit code: {code}]"));
            }
        }
    }
    Ok(out)
}

#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process(_pid: u32) {}

#[cfg(unix)]
fn synthetic_status() -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(137)
}

#[cfg(not(unix))]
fn synthetic_status() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}

// ReadTool

/// Read a file (optionally truncated).
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read a file from disk and return its contents."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Absolute or cwd-relative path"},
                "max_bytes": {"type": "integer", "description": "Truncate to this many bytes"}
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            max_bytes: Option<usize>,
        }
        let args: Args = serde_json::from_value(input)?;
        let path = resolve_path(&args.path, ctx.cwd.as_deref());
        let mut buf = tokio::fs::read(&path)
            .await
            .map_err(|e| anyhow::anyhow!("read {} failed: {e}", path.display()))?;
        if let Some(max) = args.max_bytes {
            buf.truncate(max);
        }
        Ok(ToolOutput::ok(String::from_utf8_lossy(&buf)))
    }
}

// WriteTool

/// Write a file to disk, creating parent directories as needed.
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Write a file to disk, creating parent dirs if needed."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Absolute or cwd-relative path"},
                "content": {"type": "string", "description": "File content"}
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }
        let args: Args = serde_json::from_value(input)?;
        let path = resolve_path(&args.path, ctx.cwd.as_deref());
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let bytes = args.content.len();
        tokio::fs::write(&path, args.content.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("write {} failed: {e}", path.display()))?;
        Ok(ToolOutput::ok(format!(
            "wrote {bytes} bytes to {}",
            path.display()
        )))
    }
}

// FetchTool

/// Fetch a URL (HTTP GET) and return the body.
pub struct FetchTool {
    client: reqwest::Client,
}

impl Default for FetchTool {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "fetch"
    }
    fn description(&self) -> &str {
        "Fetch a URL via HTTP GET and return body (truncated to 50KB)."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"url": {"type": "string", "description": "https:// example"}},
            "required": ["url"]
        })
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            url: String,
        }
        let args: Args = serde_json::from_value(input)?;
        let resp = self
            .client
            .get(&args.url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("fetch failed: {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("fetch body: {e}"))?;
        let truncated = if body.len() > 50_000 {
            let mut t = body[..50_000].to_string();
            t.push_str("\n[...truncated]");
            t
        } else {
            body
        };
        Ok(ToolOutput::ok(format!("[{status}]\n{truncated}")))
    }
}

// MemoryTool

/// Read/write notes in the agent's persistent memory.
pub struct MemoryTool;

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory"
    }
    fn description(&self) -> &str {
        "Read or write a memory note. action='read'|'write'|'list'|'delete'."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["read", "write", "list", "delete"]},
                "key": {"type": "string", "description": "Note key"},
                "value": {"type": "string", "description": "Note value (write only)"}
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            key: Option<String>,
            value: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        let Some(memory) = &ctx.memory else {
            return Ok(ToolOutput::err(
                "memory backend not configured in this session",
            ));
        };
        let key = || {
            args.key
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing `key`"))
        };
        match args.action.as_str() {
            "read" => Ok(ToolOutput::ok(
                memory
                    .read_note(key()?)?
                    .unwrap_or_else(|| "(no note)".to_string()),
            )),
            "write" => {
                let value = args
                    .value
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("memory.write requires `value`"))?;
                memory.write_note(key()?, value)?;
                Ok(ToolOutput::ok("ok".to_string()))
            }
            "list" => {
                let body = memory
                    .list_notes()?
                    .into_iter()
                    .map(|(k, v)| format!("- {k}: {}", truncate(&v, 80)))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(ToolOutput::ok(body))
            }
            "delete" => {
                memory.delete_note(key()?)?;
                Ok(ToolOutput::ok("ok".to_string()))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

// helpers

fn resolve_path(path: &str, cwd: Option<&std::path::Path>) -> std::path::PathBuf {
    let p = std::path::PathBuf::from(path);
    if p.is_absolute() {
        p
    } else if let Some(c) = cwd {
        c.join(p)
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut t = s[..max].to_string();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_schema_has_command() {
        let s = BashTool.schema();
        assert_eq!(s["properties"]["command"]["type"], "string");
    }

    #[tokio::test]
    async fn read_and_write_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            cwd: Some(tmp.path().to_path_buf()),
            ..Default::default()
        };
        let write = WriteTool;
        let out = write
            .execute(json!({"path": "hello.txt", "content": "world"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("wrote"));

        let read = ReadTool;
        let out = read
            .execute(json!({"path": "hello.txt"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out.content, "world");
    }

    #[test]
    fn memory_tool_no_backend() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let tool = MemoryTool;
            let out = tool
                .execute(json!({"action": "list"}), &ToolContext::default())
                .await
                .unwrap();
            assert!(out.is_error);
        });
    }
}

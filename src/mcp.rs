//! Minimal MCP stdio client: JSON-RPC 2.0 over a child process's
//! stdin/stdout, one JSON object per line. Only `initialize`,
//! `tools/list`, `tools/call` are spoken — no extra deps beyond
//! `tokio::process` + `serde_json`.
//!
//! [`McpTool`] wraps one remote tool as `mcp_<server>_<tool>`, and
//! [`build_mcp_tools`] discovers them (bootstrap wiring deferred, see TODO).

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::config::{McpConfig, McpServerConfig};
use crate::error::Result;

/// Default per-RPC timeout for MCP stdio servers.
const MCP_RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// Remote tool descriptor returned by `tools/list`.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    /// Remote tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool input.
    pub input_schema: Value,
}

/// Encode a JSON-RPC 2.0 request as one newline-terminated line.
pub fn encode_request(id: u64, method: &str, params: Value) -> String {
    let v = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    format!("{v}\n")
}

/// Decode one JSON-RPC response line, returning `result` or the `error`.
pub fn decode_response_line(line: &str) -> Result<Value> {
    let v: Value =
        serde_json::from_str(line).map_err(|e| anyhow::anyhow!("mcp bad response: {e}"))?;
    if let Some(err) = v.get("error") {
        return Err(anyhow::anyhow!("mcp error: {err}"));
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("mcp response missing result"))
}

/// Local name for a remote tool: `mcp_<server>_<tool>`, sanitized to
/// `[a-z0-9_]` so it is a stable tool id.
pub fn mcp_tool_name(server: &str, tool: &str) -> String {
    fn clean(s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect()
    }
    format!("mcp_{}_{}", clean(server), clean(tool))
}

/// One RPC round-trip: spawn `command [args...]`, send one request line,
/// read one response line, then kill the child. Stateless per call, so no
/// session tracking; servers that want a persistent session still answer
/// after the inline `initialize` in [`list_remote_tools`].
async fn rpc_once(cfg: &McpServerConfig, method: &str, params: Value) -> Result<Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    if cfg.command.trim().is_empty() {
        anyhow::bail!("mcp server command is empty");
    }
    let mut child = tokio::process::Command::new(&cfg.command)
        .args(&cfg.args)
        .envs(&cfg.env)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("mcp spawn {} failed: {e}", cfg.command))?;
    let outcome = async {
        let Some(mut writer) = child.stdin.take() else {
            return Err(anyhow::anyhow!("mcp no stdin"));
        };
        let Some(stdout) = child.stdout.take() else {
            return Err(anyhow::anyhow!("mcp no stdout"));
        };
        let mut reader = BufReader::new(stdout);
        let request = encode_request(1, method, params);
        if let Err(e) = writer.write_all(request.as_bytes()).await {
            return Err(anyhow::anyhow!("mcp write: {e}"));
        }
        writer.flush().await.ok();
        let mut line = String::new();
        if let Err(e) = reader.read_line(&mut line).await {
            return Err(anyhow::anyhow!("mcp read: {e}"));
        }
        if line.trim().is_empty() {
            return Err(anyhow::anyhow!("mcp empty response"));
        }
        decode_response_line(&line)
    }
    .await;
    let _ = child.kill().await;
    tokio::time::timeout(MCP_RPC_TIMEOUT, child.wait())
        .await
        .map_err(|_| anyhow::anyhow!("mcp wait timed out"))?
        .map_err(|e| anyhow::anyhow!("mcp wait: {e}"))?;
    outcome
}

/// `initialize` params identifying this client.
fn init_params() -> Value {
    json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "zero-hermes", "version": "0.1.0"}})
}

/// List a server's remote tools (sends `initialize`, then `tools/list`).
pub async fn list_remote_tools(cfg: &McpServerConfig) -> Result<Vec<McpToolInfo>> {
    let _ = rpc_once(cfg, "initialize", init_params()).await;
    let result = rpc_once(cfg, "tools/list", json!({})).await?;
    let Some(arr) = result.get("tools").and_then(|v| v.as_array()) else {
        return Err(anyhow::anyhow!("mcp tools/list missing tools[]"));
    };
    let mut out = Vec::with_capacity(arr.len());
    for t in arr {
        let name = t
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mcp tool missing name"))?
            .to_string();
        out.push(McpToolInfo {
            name,
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            input_schema: t
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"})),
        });
    }
    Ok(out)
}

/// Call a remote tool (sends `initialize`, then `tools/call`).
pub async fn call_remote_tool(cfg: &McpServerConfig, tool: &str, args: Value) -> Result<String> {
    let _ = rpc_once(cfg, "initialize", init_params()).await;
    let result = rpc_once(cfg, "tools/call", json!({"name": tool, "arguments": args})).await?;
    let texts: Vec<String> = result
        .get("content")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|b| b.get("text").and_then(|v| v.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if texts.is_empty() {
        Ok(result.to_string())
    } else {
        Ok(texts.join("\n"))
    }
}

/// Local [`Tool`] wrapper around one remote MCP tool.
pub struct McpTool {
    server: String,
    local_name: String,
    info: McpToolInfo,
    server_cfg: McpServerConfig,
}

impl McpTool {
    /// Wrap a remote tool for local dispatch.
    pub fn new(server: impl Into<String>, info: McpToolInfo, server_cfg: McpServerConfig) -> Self {
        let server: String = server.into();
        let local_name = mcp_tool_name(&server, &info.name);
        Self {
            server,
            local_name,
            info,
            server_cfg,
        }
    }

    /// Precomputed registry name (`mcp_<server>_<tool>`).
    pub fn local_name(&self) -> &str {
        &self.local_name
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.local_name
    }
    fn description(&self) -> &str {
        &self.info.description
    }
    fn schema(&self) -> Value {
        self.info.input_schema.clone()
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        match call_remote_tool(&self.server_cfg, &self.info.name, input).await {
            Ok(text) => Ok(ToolOutput::ok(format!("[{}] {text}", self.server))),
            Err(e) => Ok(ToolOutput::err(format!(
                "mcp {} failed: {e}",
                self.info.name
            ))),
        }
    }
}

/// Turn a `tools/list` result into local tools (not wired into bootstrap
/// yet — Wave D owns the registry shape; wire via `insert_always` once the
/// async discovery story is decided).
pub fn build_mcp_tools_from_list(
    server: &str,
    cfg: &McpServerConfig,
    tools: Vec<McpToolInfo>,
) -> Vec<McpTool> {
    tools
        .into_iter()
        .map(|info| McpTool::new(server, info, cfg.clone()))
        .collect()
}

/// Discover tools on every configured server, skipping failures.
///
/// One stdio spawn per server; a server that fails discovery is skipped
/// with a warning so one broken entry cannot take the registry down.
/// Async discovery is why bootstrap does not call this yet (see TODO).
pub async fn build_mcp_tools(cfg: &McpConfig) -> Vec<McpTool> {
    let mut names: Vec<&String> = cfg.servers.keys().collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let server_cfg = &cfg.servers[name];
        match list_remote_tools(server_cfg).await {
            Ok(infos) => out.extend(build_mcp_tools_from_list(name, server_cfg, infos)),
            Err(e) => tracing::warn!(server = %name, error = %e, "mcp discovery failed; skipping"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_framing_round_trips() {
        let line = encode_request(7, "tools/list", json!({}));
        assert!(line.ends_with('\n'));
        let v: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 7);
        assert_eq!(v["method"], "tools/list");
        let ok = decode_response_line(r#"{"jsonrpc":"2.0","id":7,"result":{"tools":[]}}"#).unwrap();
        assert_eq!(ok["tools"], json!([]));
        let err = r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"nope"}}"#;
        assert!(decode_response_line(err).is_err());
    }

    #[test]
    fn tool_names_are_sanitized() {
        assert_eq!(mcp_tool_name("fs", "read-file"), "mcp_fs_read_file");
        assert_eq!(
            mcp_tool_name("My Server!", "Read File"),
            "mcp_my_server__read_file"
        );
    }

    #[test]
    fn mcp_config_parses_from_toml() {
        let raw = "[mcp.servers.filesystem]\ncommand = \"npx\"\nargs = [\"-y\", \"srv\"]\n";
        let cfg: crate::config::Config = toml::from_str(raw).unwrap();
        let srv = cfg.mcp.servers.get("filesystem").unwrap();
        assert_eq!(srv.command, "npx");
        assert_eq!(srv.args, vec!["-y", "srv"]);
        assert_eq!(mcp_tool_name("filesystem", "read"), "mcp_filesystem_read");
    }

    #[tokio::test]
    async fn empty_command_errors_without_spawn() {
        let cfg = McpServerConfig::default();
        assert!(rpc_once(&cfg, "tools/list", json!({})).await.is_err());
    }

    #[tokio::test]
    async fn build_mcp_tools_empty_config_yields_no_tools() {
        assert!(build_mcp_tools(&McpConfig::default()).await.is_empty());
    }
}

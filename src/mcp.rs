//! Minimal MCP stdio client: JSON-RPC 2.0 over a child process's
//! stdin/stdout, one JSON object per line. Only `initialize`,
//! `tools/list`, `tools/call` are spoken — no extra deps beyond
//! `tokio::process` + `serde_json`.
//!
//! [`McpSession`] keeps one server process alive per configured server
//! (handshake once, respawn on failure). [`McpTool`] wraps one remote tool as
//! `mcp_<server>_<tool>`, and [`build_mcp_tools`] discovers them at startup.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::config::{McpConfig, McpServerConfig};
use crate::error::Result;

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

/// Per-RPC timeouts. `initialize`/`tools/list` get headroom because
/// `npx`-style servers download and start cold; tool calls can legitimately
/// run for a while.
const MCP_INIT_TIMEOUT: Duration = Duration::from_secs(60);
const MCP_CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// A live server process plus its pipes.
struct Live {
    // Held so `kill_on_drop` reaps the process when the session dies.
    _child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::BufReader<tokio::process::ChildStdout>,
}

/// Connection state for a streamable-HTTP server.
#[derive(Default)]
struct HttpState {
    client: Option<reqwest::Client>,
    session_id: Option<String>,
    ready: bool,
}

/// A persistent session with one MCP server, over stdio or streamable HTTP.
///
/// The process is spawned lazily, handshaken once (`initialize` +
/// `notifications/initialized`) and reused for every later request, so a slow
/// `npx` start is paid once instead of per tool call. Requests are serialised
/// by a mutex. Any I/O failure or timeout drops the process; the next request
/// respawns it, so a crashed or wedged server heals itself.
pub struct McpSession {
    cfg: McpServerConfig,
    live: tokio::sync::Mutex<Option<Live>>,
    http: tokio::sync::Mutex<HttpState>,
    next_id: std::sync::atomic::AtomicU64,
}

impl McpSession {
    /// New (not yet started) session for `cfg`.
    pub fn new(cfg: McpServerConfig) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            cfg,
            live: tokio::sync::Mutex::new(None),
            http: tokio::sync::Mutex::new(HttpState::default()),
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    fn fresh_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Spawn the server and complete the MCP handshake.
    async fn start(&self) -> Result<Live> {
        use tokio::io::{AsyncWriteExt, BufReader};
        if self.cfg.command.trim().is_empty() {
            anyhow::bail!("mcp server command is empty");
        }
        let mut child = tokio::process::Command::new(&self.cfg.command)
            .args(&self.cfg.args)
            .envs(&self.cfg.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("mcp spawn {} failed: {e}", self.cfg.command))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("mcp no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("mcp no stdout"))?;
        let mut live = Live {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        round_trip(
            &mut live,
            self.fresh_id(),
            "initialize",
            init_params(),
            MCP_INIT_TIMEOUT,
        )
        .await?;
        let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        live.stdin
            .write_all(format!("{note}\n").as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("mcp write: {e}"))?;
        live.stdin.flush().await.ok();
        Ok(live)
    }

    /// Send one request and return its `result`, (re)starting the server as
    /// needed. A request that fails on a *reused* process is retried once on
    /// a fresh one, since the likeliest cause is a server that died idle.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        if self.cfg.url.is_some() {
            return self.request_http(method, params).await;
        }
        let timeout = if method == "tools/call" {
            MCP_CALL_TIMEOUT
        } else {
            MCP_INIT_TIMEOUT
        };
        let mut guard = self.live.lock().await;
        for attempt in 0..2 {
            let reused = guard.is_some();
            if guard.is_none() {
                *guard = Some(self.start().await?);
            }
            let Some(live) = guard.as_mut() else {
                anyhow::bail!("mcp session unavailable");
            };
            match round_trip(live, self.fresh_id(), method, params.clone(), timeout).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let protocol_error = e.to_string().starts_with("mcp error:");
                    if !protocol_error {
                        // Transport failure: the process state is unknown.
                        *guard = None;
                    }
                    if protocol_error || !reused || attempt == 1 {
                        return Err(e);
                    }
                }
            }
        }
        anyhow::bail!("mcp request failed")
    }

    /// Streamable-HTTP flavour of [`Self::request`]: POST JSON-RPC, accept a
    /// JSON body or an SSE-framed one, carry `Mcp-Session-Id`, and redo the
    /// handshake if the server forgot the session.
    async fn request_http(&self, method: &str, params: Value) -> Result<Value> {
        let timeout = if method == "tools/call" {
            MCP_CALL_TIMEOUT
        } else {
            MCP_INIT_TIMEOUT
        };
        let mut st = self.http.lock().await;
        for attempt in 0..2 {
            let was_ready = st.ready;
            if !st.ready {
                let init = self.fresh_id();
                http_send(
                    &self.cfg,
                    &mut st,
                    Some(init),
                    "initialize",
                    init_params(),
                    MCP_INIT_TIMEOUT,
                )
                .await?;
                http_send(
                    &self.cfg,
                    &mut st,
                    None,
                    "notifications/initialized",
                    json!({}),
                    MCP_INIT_TIMEOUT,
                )
                .await?;
                st.ready = true;
            }
            let id = self.fresh_id();
            match http_send(
                &self.cfg,
                &mut st,
                Some(id),
                method,
                params.clone(),
                timeout,
            )
            .await
            {
                Ok(v) => return Ok(v),
                Err(e) => {
                    let protocol_error = e.to_string().starts_with("mcp error:");
                    if !protocol_error {
                        st.ready = false;
                        st.session_id = None;
                    }
                    if protocol_error || !was_ready || attempt == 1 {
                        return Err(e);
                    }
                }
            }
        }
        anyhow::bail!("mcp request failed")
    }

    /// `tools/list`.
    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>> {
        let result = self.request("tools/list", json!({})).await?;
        parse_tool_list(&result)
    }

    /// `tools/call`, flattening text content blocks.
    pub async fn call_tool(&self, tool: &str, args: Value) -> Result<String> {
        let result = self
            .request("tools/call", json!({"name": tool, "arguments": args}))
            .await?;
        let text = flatten_call_result(&result);
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            anyhow::bail!("{text}");
        }
        Ok(text)
    }
}

/// One streamable-HTTP POST. `id: None` sends a notification (no reply read).
async fn http_send(
    cfg: &McpServerConfig,
    st: &mut HttpState,
    id: Option<u64>,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value> {
    let Some(url) = cfg.url.as_deref() else {
        anyhow::bail!("mcp server has no url");
    };
    if st.client.is_none() {
        st.client = Some(
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| anyhow::anyhow!("mcp http client: {e}"))?,
        );
    }
    let Some(client) = st.client.clone() else {
        anyhow::bail!("mcp http client unavailable");
    };
    let body = match id {
        Some(id) => json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        None => json!({"jsonrpc": "2.0", "method": method, "params": params}),
    };
    let mut req = client
        .post(url)
        .header("accept", "application/json, text/event-stream")
        .json(&body);
    for (k, v) in &cfg.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    if let Some(sid) = &st.session_id {
        req = req.header("mcp-session-id", sid.as_str());
    }
    let exchange = async {
        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("mcp http request: {e}"))?;
        if let Some(sid) = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            st.session_id = Some(sid.to_string());
        }
        let status = resp.status();
        let is_sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.contains("text/event-stream"));
        let text = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("mcp http body: {e}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "mcp http {status}: {}",
                crate::util::truncate_bytes(&text, 256)
            );
        }
        let Some(id) = id else {
            return Ok(Value::Null);
        };
        let candidates: Vec<String> = if is_sse {
            sse_data_events(&text)
        } else {
            vec![text]
        };
        for c in candidates {
            let Ok(v) = serde_json::from_str::<Value>(c.trim()) else {
                continue;
            };
            let items = match v {
                Value::Array(a) => a,
                other => vec![other],
            };
            for item in items {
                if item.get("id").and_then(Value::as_u64) == Some(id)
                    && item.get("method").is_none()
                {
                    return decode_response_line(&item.to_string());
                }
            }
        }
        anyhow::bail!("mcp http response had no reply for request {id}")
    };
    match tokio::time::timeout(timeout, exchange).await {
        Ok(r) => r,
        Err(_) => anyhow::bail!("mcp {method} timed out after {}s", timeout.as_secs()),
    }
}

/// Join the `data:` lines of each blank-line-delimited SSE event.
fn sse_data_events(body: &str) -> Vec<String> {
    let mut events = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                events.push(cur.join("\n"));
                cur.clear();
            }
        } else if let Some(d) = line.strip_prefix("data:") {
            cur.push(d.strip_prefix(' ').unwrap_or(d));
        }
    }
    if !cur.is_empty() {
        events.push(cur.join("\n"));
    }
    events
}

/// Write one request and read lines until the response with the same id,
/// skipping server notifications, server-initiated requests and log noise.
async fn round_trip(
    live: &mut Live,
    id: u64,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let request = encode_request(id, method, params);
    let exchange = async {
        live.stdin
            .write_all(request.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("mcp write: {e}"))?;
        live.stdin.flush().await.ok();
        let mut line = String::new();
        loop {
            line.clear();
            let n = live
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| anyhow::anyhow!("mcp read: {e}"))?;
            if n == 0 {
                anyhow::bail!("mcp server closed its output");
            }
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue; // stray log line on stdout
            };
            if v.get("id").and_then(Value::as_u64) != Some(id) || v.get("method").is_some() {
                continue; // notification, server request, or stale response
            }
            return decode_response_line(line.trim());
        }
    };
    match tokio::time::timeout(timeout, exchange).await {
        Ok(r) => r,
        Err(_) => anyhow::bail!("mcp {method} timed out after {}s", timeout.as_secs()),
    }
}

/// `initialize` params identifying this client.
fn init_params() -> Value {
    json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "zero-hermes", "version": env!("CARGO_PKG_VERSION")}})
}

fn parse_tool_list(result: &Value) -> Result<Vec<McpToolInfo>> {
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

fn flatten_call_result(result: &Value) -> String {
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
        result.to_string()
    } else {
        texts.join("\n")
    }
}

/// One-shot: list a server's tools on a throwaway session.
pub async fn list_remote_tools(cfg: &McpServerConfig) -> Result<Vec<McpToolInfo>> {
    McpSession::new(cfg.clone()).list_tools().await
}

/// One-shot: call a tool on a throwaway session.
pub async fn call_remote_tool(cfg: &McpServerConfig, tool: &str, args: Value) -> Result<String> {
    McpSession::new(cfg.clone()).call_tool(tool, args).await
}

/// Local [`Tool`] wrapper around one remote MCP tool.
pub struct McpTool {
    server: String,
    local_name: String,
    info: McpToolInfo,
    session: std::sync::Arc<McpSession>,
}

impl McpTool {
    /// Wrap a remote tool, giving it a private session.
    pub fn new(server: impl Into<String>, info: McpToolInfo, server_cfg: McpServerConfig) -> Self {
        Self::with_session(server, info, McpSession::new(server_cfg))
    }

    /// Wrap a remote tool on a session shared with the server's other tools.
    pub fn with_session(
        server: impl Into<String>,
        info: McpToolInfo,
        session: std::sync::Arc<McpSession>,
    ) -> Self {
        let server: String = server.into();
        let local_name = mcp_tool_name(&server, &info.name);
        Self {
            server,
            local_name,
            info,
            session,
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
        match self.session.call_tool(&self.info.name, input).await {
            Ok(text) => Ok(ToolOutput::ok(format!("[{}] {text}", self.server))),
            Err(e) => Ok(ToolOutput::err(format!(
                "mcp {} failed: {e}",
                self.info.name
            ))),
        }
    }
}

/// Turn a `tools/list` result into local tools that share one live session
/// (wired into bootstrap via `bootstrap::build_tool_registry_async`).
pub fn build_mcp_tools_from_list(
    server: &str,
    cfg: &McpServerConfig,
    tools: Vec<McpToolInfo>,
) -> Vec<McpTool> {
    let session = McpSession::new(cfg.clone());
    tools
        .into_iter()
        .map(|info| McpTool::with_session(server, info, session.clone()))
        .collect()
}

/// Sync hint of configured MCP servers (no discovery, no I/O).
///
/// Returns sorted server names (`cfg.mcp.server_names()`). The sync tool
/// registry cannot perform async stdio discovery, so no `mcp_*` tools are
/// advertised until `build_mcp_tools` is wired at startup; use this hint
/// for prompt diagnostics instead of claiming tools that do not exist.
pub fn mcp_tool_names(cfg: &McpConfig) -> Vec<String> {
    cfg.server_names()
}

/// Discover tools on every configured server, skipping failures.
///
/// One stdio spawn per server; a server that fails discovery is skipped
/// with a warning so one broken entry cannot take the registry down.
/// Called at startup by `bootstrap::build_tool_registry_async`.
pub async fn build_mcp_tools(cfg: &McpConfig) -> Vec<McpTool> {
    let mut names: Vec<&String> = cfg.servers.keys().collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let session = McpSession::new(cfg.servers[name].clone());
        match session.list_tools().await {
            Ok(infos) => out.extend(
                infos
                    .into_iter()
                    .map(|info| McpTool::with_session(name, info, session.clone())),
            ),
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
        assert!(cfg.mcp.is_configured());
        assert_eq!(mcp_tool_names(&cfg.mcp), vec!["filesystem".to_string()]);
        assert!(!McpConfig::default().is_configured());
        assert!(mcp_tool_names(&McpConfig::default()).is_empty());
    }

    #[test]
    fn http_server_config_parses_and_expands_env() {
        std::env::set_var("ZH_MCP_TEST_TOKEN", "tok");
        let raw = "[mcp.servers.r]\nurl = \"https://h/mcp\"\nheaders = { Authorization = \"Bearer ${ZH_MCP_TEST_TOKEN}\" }\n";
        let mut cfg: crate::config::Config = toml::from_str(raw).unwrap();
        cfg.expand_env();
        let srv = &cfg.mcp.servers["r"];
        assert_eq!(srv.url.as_deref(), Some("https://h/mcp"));
        assert_eq!(srv.headers["Authorization"], "Bearer tok");
    }

    #[test]
    fn sse_events_join_data_lines() {
        let body = "event: message\ndata: {\"a\":\ndata: 1}\n\ndata: {\"b\":2}\n\n";
        assert_eq!(sse_data_events(body), vec!["{\"a\":\n1}", "{\"b\":2}"]);
    }

    #[tokio::test]
    async fn empty_command_errors_without_spawn() {
        let cfg = McpServerConfig::default();
        assert!(McpSession::new(cfg)
            .request("tools/list", json!({}))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn build_mcp_tools_empty_config_yields_no_tools() {
        assert!(build_mcp_tools(&McpConfig::default()).await.is_empty());
    }
}

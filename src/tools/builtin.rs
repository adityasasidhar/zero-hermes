//! Built-in tools: `bash`, `read`, `write`, `fetch`, `memory`, `subagent`.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::error::Result;
use crate::util::truncate_bytes;

/// Default per-stream byte cap (1 MiB). Commands that flood stdout/stderr
/// past this get their reader threads disconnected so the child receives
/// SIGPIPE on the next write and exits.
const BASH_OUTPUT_CAP: usize = 1 << 20;

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
        // Note: `Command::kill_on_drop` is nightly-only on stable, so we
        // rely on `run_with_timeout`'s SIGKILL on timeout plus the runtime
        // reaping the child via the wait thread.
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

/// Drain `reader` into `buf` until EOF or until `cap` bytes have been
/// collected. Returns `true` if the cap was hit (the caller should treat
/// the output as truncated and disconnect the pipe to provoke SIGPIPE).
fn read_capped<R: std::io::Read>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    cap: usize,
) -> std::io::Result<bool> {
    use std::io::Read;
    let mut limited = reader.take(cap as u64 + 1);
    limited.read_to_end(buf)?;
    Ok(buf.len() > cap)
}

/// Run a command with a timeout. If the timeout expires, the child is killed.
fn run_with_timeout(mut cmd: std::process::Command, timeout: Duration) -> std::io::Result<String> {
    use std::sync::mpsc;
    use std::thread;

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let t_out = thread::spawn(move || {
        let mut buf = Vec::with_capacity(4096);
        let truncated = out_pipe
            .as_mut()
            .map(|s| read_capped(s, &mut buf, BASH_OUTPUT_CAP).unwrap_or(false))
            .unwrap_or(false);
        (buf, truncated)
    });
    let t_err = thread::spawn(move || {
        let mut buf = Vec::with_capacity(1024);
        let truncated = err_pipe
            .as_mut()
            .map(|s| read_capped(s, &mut buf, BASH_OUTPUT_CAP).unwrap_or(false))
            .unwrap_or(false);
        (buf, truncated)
    });
    let wait_thread = thread::spawn(move || {
        let r = child.wait();
        let _ = tx.send(r);
    });

    let (status, timed_out) = match rx.recv_timeout(timeout) {
        Ok(r) => (r, false),
        Err(_) => {
            kill_process(pid);
            let r = rx.recv_timeout(Duration::from_secs(2));
            // If the child still hasn't died after 2s, give up on its
            // status; it may be unkillable.
            (r.unwrap_or_else(|_| Ok(synthetic_status())), true)
        }
    };
    let _ = wait_thread.join();

    let (out_buf, out_truncated) = t_out.join().unwrap_or_default();
    let (err_buf, err_truncated) = t_err.join().unwrap_or_default();
    let mut out = String::from_utf8_lossy(&out_buf).into_owned();
    let err = String::from_utf8_lossy(&err_buf);
    if !err.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&err);
    }
    if out_truncated || err_truncated {
        let what = if out_truncated && err_truncated {
            "stdout+stderr"
        } else if out_truncated {
            "stdout"
        } else {
            "stderr"
        };
        out.push_str(&format!(
            "\n[output truncated at {} bytes; {what} capped]",
            BASH_OUTPUT_CAP
        ));
    }
    if timed_out {
        out.push_str(&format!("\n[timeout after {:?}]", timeout));
    } else if let Ok(s) = status {
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
    std::process::ExitStatus::from_raw(124)
}

#[cfg(not(unix))]
fn synthetic_status() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(124)
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

/// Hard cap on fetched response size (50 KiB). Bodies larger than this
/// are truncated and the rest of the stream is dropped so a hostile or
/// huge URL can't OOM the process.
const FETCH_BODY_CAP: usize = 50 * 1024;

/// Fetch a URL (HTTP GET) and return readable text.
///
/// The response body is converted from HTML to plain text (scripts,
/// styles and tags stripped, entities decoded) and truncated to 50KB.
/// Private/loopback hosts are blocked as SSRF protection unless the
/// tool was built with [`FetchTool::with_private_allowed`].
pub struct FetchTool {
    client: reqwest::Client,
    allow_private: bool,
}

impl FetchTool {
    /// Build a fetch tool that permits private/loopback hosts.
    ///
    /// Used by tests that spin up a local HTTP server on 127.0.0.1.
    /// Production code should use [`FetchTool::default`], which blocks them.
    pub fn with_private_allowed() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "fetch client build failed; using default client");
                reqwest::Client::new()
            });
        Self {
            client,
            allow_private: true,
        }
    }
}

impl Default for FetchTool {
    fn default() -> Self {
        // The release profile sets `panic = "abort"`, so an `expect` here
        // would take the whole daemon down rather than fail one tool call.
        // A default client is a fine fallback; it just has no timeout.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "fetch client build failed; using default client");
                reqwest::Client::new()
            });
        Self {
            client,
            allow_private: false,
        }
    }
}

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        "fetch"
    }
    fn description(&self) -> &str {
        "Fetch a URL via HTTP GET and return readable text (HTML stripped, truncated to 50KB). Private hosts blocked."
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
        if !self.allow_private && is_fetch_blocked(&args.url) {
            return Ok(ToolOutput::err(format!(
                "fetch blocked: refusing to fetch {:?} (private/local host or non-http(s) scheme)",
                truncate_bytes(&args.url, 200)
            )));
        }
        let resp = self
            .client
            .get(&args.url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("fetch failed: {e}"))?;
        let status = resp.status();
        // Stream the body, stopping after FETCH_BODY_CAP bytes so we
        // don't buffer arbitrarily large responses into memory.
        use futures::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buf = Vec::with_capacity(FETCH_BODY_CAP.min(16 * 1024));
        let mut truncated = false;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("fetch body: {e}"))?;
            if buf.len() + chunk.len() > FETCH_BODY_CAP {
                let room = FETCH_BODY_CAP - buf.len();
                buf.extend_from_slice(&chunk[..room]);
                truncated = true;
                break;
            }
            buf.extend_from_slice(&chunk);
        }
        let raw = String::from_utf8_lossy(&buf).into_owned();
        let text = html_to_text(&raw);
        let body = if truncated {
            format!("{text}\n[...truncated at {} bytes]", FETCH_BODY_CAP)
        } else {
            text
        };
        Ok(ToolOutput::ok(format!("[{status}]\n{body}")))
    }
}

/// Return `true` when `url` must not be fetched (SSRF guard).
///
/// Blocks anything that is not `http(s)` plus loopback/private/link-local
/// hosts matched by string only (no DNS resolution): `localhost`,
/// `127.*`, `10.*`, `192.168.*`, `172.16-31.*`, `169.254.*`, `0.*`,
/// `::1`, `0.0.0.0`, and `.local` / `.internal` / `.localhost` suffixes.
pub fn is_fetch_blocked(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return true;
    }
    let Some(host) = fetch_host(url) else {
        return true;
    };
    if host.is_empty() {
        return true;
    }
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return true;
    }
    if host == "0.0.0.0" || host == "::1" || host == "0:0:0:0:0:0:0:1" {
        return true;
    }
    if is_blocked_ipv4(&host) {
        return true;
    }
    false
}

/// Extract the lowercase host from an `http(s)` URL without new deps.
///
/// Strips userinfo (`user@host`), port (`:port`) and `[brackets]`.
fn fetch_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://")?.1;
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    // Strip userinfo: `http://user:pass@host/`.
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(stripped) = hostport.strip_prefix('[') {
        stripped.split(']').next().unwrap_or(stripped)
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    Some(host)
}

/// String-only IPv4 private/loopback check.
fn is_blocked_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    let mut octets = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        match part.parse::<u16>() {
            Ok(n) if n <= 255 => octets[i] = n as u8,
            _ => return false,
        }
    }
    match octets {
        [127, _, _, _] => true,
        [10, _, _, _] => true,
        [192, 168, _, _] => true,
        [172, b, _, _] if (16..=31).contains(&b) => true,
        [169, 254, _, _] => true,
        [0, _, _, _] => true,
        _ => false,
    }
}

/// Convert HTML into readable plain text.
///
/// Strips `<script>`/`<style>` blocks and comments, turns block-level
/// tags into line breaks, drops remaining tags, decodes common entities
/// and collapses whitespace.
pub fn html_to_text(html: &str) -> String {
    let without_scripts = strip_tag_block(html, "script");
    let without_styles = strip_tag_block(&without_scripts, "style");
    let without_comments = strip_html_comments(&without_styles);
    let with_breaks = insert_block_breaks(&without_comments);
    let stripped = strip_all_tags(&with_breaks);
    let decoded = decode_entities(&stripped);
    collapse_text(&decoded)
}

/// Remove `<tag ...>...</tag>` blocks case-insensitively.
fn strip_tag_block(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open_pat = format!("<{tag}");
    let close_pat = format!("</{tag}");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let mut lower_rest = lower.as_str();
    loop {
        let Some(open) = lower_rest.find(open_pat.as_str()) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open]);
        let after_open = &rest[open..];
        let Some(tag_end) = after_open.find('>') else {
            break;
        };
        let search_from = open + tag_end + 1;
        let Some(close_rel) = lower_rest[search_from..].find(close_pat.as_str()) else {
            break;
        };
        let close_abs = search_from + close_rel;
        let tail = &rest[close_abs..];
        let Some(close_end) = tail.find('>') else {
            break;
        };
        rest = &rest[close_abs + close_end + 1..];
        lower_rest = &lower_rest[close_abs + close_end + 1..];
        out.push('\n');
    }
    out
}

/// Remove `<!-- ... -->` comments.
fn strip_html_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let Some(open) = rest.find("<!--") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open]);
        let after = &rest[open + 4..];
        let Some(close) = after.find("-->") else {
            break;
        };
        rest = &after[close + 3..];
        out.push(' ');
    }
    out
}

/// Replace block-level tags with newlines so paragraphs stay separated.
fn insert_block_breaks(html: &str) -> String {
    const BLOCKS: &[&str] = &[
        "br",
        "p",
        "div",
        "li",
        "ul",
        "ol",
        "tr",
        "table",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "blockquote",
        "hr",
        "pre",
        "section",
        "article",
    ];
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag = &rest[open..];
        let Some(end) = tag.find('>') else {
            out.push(' ');
            break;
        };
        let inner = tag[1..end].trim_start_matches('/').trim_start();
        let name_len = inner
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(inner.len());
        let name = inner[..name_len].to_ascii_lowercase();
        if BLOCKS.contains(&name.as_str()) {
            out.push('\n');
        } else {
            out.push(' ');
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Drop any remaining `<...>` tags (already spaced by the previous pass).
fn strip_all_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag = &rest[open..];
        match tag.find('>') {
            Some(end) => {
                rest = &tag[end + 1..];
            }
            None => {
                out.push_str(tag);
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode common HTML entities plus decimal/hex numeric references.
fn decode_entities(s: &str) -> String {
    let out = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    // Numeric references like `&#65;` / `&#x41;`.
    let mut result = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(start) = rest.find("&#") {
        result.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let Some(semi) = tail.find(';') else {
            result.push_str(&rest[start..]);
            break;
        };
        let entity = &tail[..semi];
        let decoded = if let Some(hex) = entity
            .strip_prefix('x')
            .or_else(|| entity.strip_prefix('X'))
        {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else {
            entity.parse::<u32>().ok().and_then(char::from_u32)
        };
        match decoded {
            Some(ch) => result.push(ch),
            None => {
                result.push_str("&#");
                result.push_str(entity);
                result.push(';');
            }
        }
        rest = &tail[semi + 1..];
    }
    result.push_str(rest);
    result
}

/// Collapse whitespace: trim lines, squeeze inner runs, drop empties.
fn collapse_text(s: &str) -> String {
    let mut lines = Vec::new();
    for line in s.replace('\r', "\n").split('\n') {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !collapsed.is_empty() {
            lines.push(collapsed);
        }
    }
    let joined = lines.join("\n");
    truncate_bytes(&joined, FETCH_BODY_CAP)
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
                "action": {"type": "string", "enum": ["read", "write", "list", "delete", "search"]},
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
                    .map(|(k, v)| format!("- {k}: {}", truncate_bytes(&v, 80)))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(ToolOutput::ok(body))
            }
            "search" => {
                let query = key()?;
                let hits = memory.search_history(query, 8)?;
                Ok(ToolOutput::ok(
                    hits.into_iter()
                        .map(|hit| {
                            format!("- [{}] {}", hit.session_id, truncate_bytes(&hit.text, 500))
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                ))
            }
            "delete" => {
                memory.delete_note(key()?)?;
                Ok(ToolOutput::ok("ok".to_string()))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

// CronTool

/// Note key holding the agent-managed custom cron job list (JSON).
pub const CRON_CUSTOM_NOTE_KEY: &str = "cron:custom";

/// One agent-managed cron job, persisted as JSON in a memory note.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomCronJob {
    /// Unique name.
    pub name: String,
    /// 5-field cron expression.
    pub schedule: String,
    /// Prompt sent to the agent when the job fires.
    pub prompt: String,
}

/// Manage reminder-style cron jobs persisted in memory.
///
/// `list` / `add` / `remove` operate on the `cron:custom` memory note so
/// jobs survive restarts. Jobs from `zero_hermes.toml` are static: the
/// gateway scheduler is built once at boot, so custom jobs take effect
/// after a restart (the tool output says so). The gateway also prefixes
/// late ticks with their scheduled time (see `CronEvent.at`).
pub struct CronTool;

impl CronTool {
    /// Load custom jobs from the memory note (empty when absent/corrupt).
    fn load_custom(memory: &crate::memory::Memory) -> Vec<CustomCronJob> {
        let Ok(Some(raw)) = memory.read_note(CRON_CUSTOM_NOTE_KEY) else {
            return Vec::new();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    /// Persist custom jobs back to the memory note.
    fn save_custom(memory: &crate::memory::Memory, jobs: &[CustomCronJob]) -> Result<()> {
        let raw = serde_json::to_string(jobs)?;
        memory.write_note(CRON_CUSTOM_NOTE_KEY, &raw)?;
        Ok(())
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }
    fn description(&self) -> &str {
        "List, add, or remove custom cron jobs (persisted in memory; gateway restart required to take effect)."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "add", "remove"]},
                "name": {"type": "string", "description": "Job name (add/remove)"},
                "schedule": {"type": "string", "description": "5-field cron expression (add only)"},
                "prompt": {"type": "string", "description": "Agent prompt (add only)"}
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            name: Option<String>,
            schedule: Option<String>,
            prompt: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        let Some(memory) = &ctx.memory else {
            return Ok(ToolOutput::err(
                "memory backend not configured in this session",
            ));
        };
        match args.action.as_str() {
            "list" => {
                let jobs = Self::load_custom(memory);
                if jobs.is_empty() {
                    return Ok(ToolOutput::ok(
                        "(no custom cron jobs; TOML jobs are static and need a restart to change)",
                    ));
                }
                let body = jobs
                    .into_iter()
                    .map(|j| {
                        format!(
                            "- {} {} -> {}",
                            j.name,
                            j.schedule,
                            truncate_bytes(&j.prompt, 120)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(ToolOutput::ok(body))
            }
            "add" => {
                let name = args
                    .name
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("cron.add requires `name`"))?;
                let schedule = args
                    .schedule
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("cron.add requires `schedule`"))?;
                let prompt = args
                    .prompt
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("cron.add requires `prompt`"))?;
                if name.trim().is_empty() {
                    return Ok(ToolOutput::err("cron.add requires a non-empty `name`"));
                }
                // Validate the expression now so a typo fails fast.
                if let Err(e) = crate::cron::CronJob::new(name, schedule, prompt) {
                    return Ok(ToolOutput::err(format!("invalid schedule: {e}")));
                }
                let mut jobs = Self::load_custom(memory);
                if let Some(existing) = jobs.iter_mut().find(|j| j.name == name) {
                    existing.schedule = schedule.to_string();
                    existing.prompt = prompt.to_string();
                } else {
                    jobs.push(CustomCronJob {
                        name: name.to_string(),
                        schedule: schedule.to_string(),
                        prompt: prompt.to_string(),
                    });
                }
                Self::save_custom(memory, &jobs)?;
                Ok(ToolOutput::ok(format!(
                    "saved cron job {name:?} (takes effect after gateway restart)"
                )))
            }
            "remove" => {
                let name = args
                    .name
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("cron.remove requires `name`"))?;
                let mut jobs = Self::load_custom(memory);
                let before = jobs.len();
                jobs.retain(|j| j.name != name);
                if jobs.len() == before {
                    return Ok(ToolOutput::err(format!(
                        "no custom cron job named {name:?}"
                    )));
                }
                Self::save_custom(memory, &jobs)?;
                Ok(ToolOutput::ok(format!("removed cron job {name:?}")))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

/// Read and evolve local procedural skills. New skills are written beneath
/// the configured skills root, making the learning loop durable and auditable.
pub struct SkillTool {
    root: PathBuf,
}

impl SkillTool {
    /// Create a skill tool rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }
    fn description(&self) -> &str {
        "List, read, create, or update reusable SKILL.md procedures."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object", "properties": {
                "action": {"type": "string", "enum": ["list", "read", "write"]},
                "name": {"type": "string", "description": "skill name (letters, digits, _, -)"},
                "content": {"type": "string", "description": "full SKILL.md content for write"}
            }, "required": ["action"]
        })
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            name: Option<String>,
            content: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        match args.action.as_str() {
            "list" => Ok(ToolOutput::ok(
                crate::skills::SkillRegistry::load_dir(&self.root)?
                    .iter()
                    .map(|s| s.index_line())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            "read" => {
                let name = safe_skill_name(args.name.as_deref())?;
                Ok(ToolOutput::ok(
                    tokio::fs::read_to_string(self.root.join(name).join("SKILL.md")).await?,
                ))
            }
            "write" => {
                let name = safe_skill_name(args.name.as_deref())?;
                let content = args
                    .content
                    .ok_or_else(|| anyhow::anyhow!("skill.write requires content"))?;
                let path = self.root.join(&name).join("SKILL.md");
                tokio::fs::create_dir_all(path.parent().unwrap_or(&self.root)).await?;
                tokio::fs::write(&path, content).await?;
                Ok(ToolOutput::ok(format!("saved learned skill {name}")))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

fn safe_skill_name(value: Option<&str>) -> Result<String> {
    let name = value.ok_or_else(|| anyhow::anyhow!("missing skill name"))?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        anyhow::bail!("invalid skill name");
    }
    Ok(name.to_string())
}

// SubAgentTool

/// Spawn a fresh, isolated agent loop and return its final text answer.
///
/// This is the same pattern Hermes Agent uses for delegation: the sub-agent
/// sees no prior history, gets a role-flavored system prompt, and runs with
/// the parent's tool registry minus `subagent` (so it can't recurse).
///
/// Construct via [`SubAgentTool::new`] with the LLM provider and the parent
/// `ToolRegistry`. The tool is then registered in the parent's registry;
/// calling it spawns a child loop that uses a *clone* of the registry.
pub struct SubAgentTool {
    provider: Arc<dyn crate::agent::LlmProvider>,
    parent_registry: Arc<crate::tools::ToolRegistry>,
    limits: crate::agent::RunLimits,
}

impl SubAgentTool {
    /// Build a sub-agent tool. Pass the parent's provider + tool registry;
    /// the tool stores `Arc` clones of both.
    pub fn new(
        provider: Arc<dyn crate::agent::LlmProvider>,
        parent_registry: Arc<crate::tools::ToolRegistry>,
        limits: crate::agent::RunLimits,
    ) -> Self {
        Self {
            provider,
            parent_registry,
            limits,
        }
    }
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "subagent"
    }
    fn description(&self) -> &str {
        "Spawn a fresh isolated agent loop with a role and task. Returns the sub-agent's final answer as a string. Cannot nest."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Role label for the sub-agent (e.g. 'researcher', 'writer')."},
                "task": {"type": "string", "description": "One self-contained task."},
                "tasks": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "task": {"type": "string"}}, "required": ["name", "task"]}, "description": "Up to four independent tasks, run concurrently."},
                "timeout": {"type": "integer", "description": "Per-child timeout seconds (default 120, max 600)."}
            }
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Child {
            name: String,
            task: String,
        }
        #[derive(Deserialize)]
        struct Args {
            name: Option<String>,
            task: Option<String>,
            tasks: Option<Vec<Child>>,
            timeout: Option<u64>,
        }
        let args: Args = serde_json::from_value(input)?;
        let (children, multiple) = match args.tasks {
            Some(tasks) if !tasks.is_empty() && tasks.len() <= 4 => {
                let multiple = tasks.len() > 1;
                (tasks, multiple)
            }
            Some(_) => return Ok(ToolOutput::err("tasks must contain 1..=4 entries")),
            None => match (args.name, args.task) {
                (Some(name), Some(task)) => (vec![Child { name, task }], false),
                _ => return Ok(ToolOutput::err("provide name + task, or tasks")),
            },
        };
        let timeout = Duration::from_secs(args.timeout.unwrap_or(120).min(600));
        let futures = children.into_iter().map(|child_args| async move {
            let mut child = (*self.parent_registry).clone();
            child.remove("subagent");
            let child = Arc::new(child);
            let system = format!("You are a sub-agent with role: {}. Complete the task, use tools when needed, and return concise evidence-backed results. You have no prior conversation history.", child_args.name);
            let mut history = Vec::new();
            let run = crate::agent::run(self.provider.as_ref(), child.as_ref(), Some(&system), &mut history, &child_args.task, self.limits, ctx);
            let result = tokio::time::timeout(timeout, run).await;
            match result { Ok(Ok(text)) if !multiple => text, Ok(Ok(text)) => format!("## {}\n{}", child_args.name, text), Ok(Err(e)) => format!("## {} (failed)\n{e}", child_args.name), Err(_) => format!("## {} (timed out)", child_args.name) }
        });
        let results = futures::future::join_all(futures).await;
        Ok(ToolOutput::ok(results.join("\n\n")))
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

    #[test]
    fn fetch_blocks_private_hosts_and_non_http() {
        for url in [
            "http://localhost/x",
            "http://localhost:8080/",
            "http://127.0.0.1/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            "http://172.16.0.1/",
            "http://172.31.255.1/",
            "http://169.254.10.20/",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://service.local/",
            "http://db.internal/",
            "ftp://example.com/file",
            "file:///etc/passwd",
            "gopher://example.com/",
        ] {
            assert!(is_fetch_blocked(url), "should block {url}");
        }
        assert!(is_fetch_blocked("http://172.32.0.1/") == false);
        assert!(!is_fetch_blocked("https://example.com/page"));
        assert!(!is_fetch_blocked("http://example.com:8080/a?b=c"));
    }

    #[test]
    fn html_to_text_strips_scripts_and_tags() {
        let html = "<html><head><style>.x{color:red}</style>\
            <script>alert(1)</script></head>\
            <body><h1>Hi &amp; bye</h1><p>Hello <b>world</b></p><!-- c --><br>End</body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hi & bye"), "entities decoded: {text}");
        assert!(text.contains("Hello world"), "tags stripped: {text}");
        assert!(!text.contains("alert"), "scripts removed: {text}");
        assert!(!text.contains("color"), "styles removed: {text}");
        assert!(!text.contains('<'), "no tags left: {text}");
    }

    #[tokio::test]
    async fn fetch_tool_refuses_blocked_url_without_network() {
        let tool = FetchTool::default();
        let out = tool
            .execute(
                json!({"url": "http://127.0.0.1:9/nope"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
        assert!(out.content.contains("blocked"));
    }

    #[tokio::test]
    async fn cron_tool_crud_round_trip() {
        let mem = Arc::new(crate::memory::Memory::in_memory().unwrap());
        let ctx = ToolContext {
            memory: Some(mem),
            ..Default::default()
        };
        let tool = CronTool;
        let out = tool.execute(json!({"action": "list"}), &ctx).await.unwrap();
        assert!(out.content.contains("no custom cron jobs"));
        let out = tool
            .execute(
                json!({"action": "add", "name": "morning", "schedule": "0 9 * * *", "prompt": "say hi"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.is_error, "add failed: {}", out.content);
        let out = tool.execute(json!({"action": "list"}), &ctx).await.unwrap();
        assert!(out.content.contains("morning"));
        let out = tool
            .execute(
                json!({"action": "add", "name": "bad", "schedule": "nope", "prompt": "x"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.is_error, "bad schedule should fail");
        let out = tool
            .execute(json!({"action": "remove", "name": "morning"}), &ctx)
            .await
            .unwrap();
        assert!(!out.is_error);
        let out = tool
            .execute(json!({"action": "remove", "name": "morning"}), &ctx)
            .await
            .unwrap();
        assert!(out.is_error);
    }
}

//! Built-in tools: `bash`, `read`, `write`, `fetch`, `memory`, `skill`,
//! `subagent`, Hermes-compat aliases, and the `edit` / `search_files` /
//! `todo` / `ask` / `message` helpers.
//!
//! Hermes-compat aliases (`execute_command`, `execute_code`, `read_file`,
//! `write_file`, `search_files`, `skill_view`, `web_search`, `web_extract`)
//! exist because vendored Hermes skills reference those names; they share
//! the same implementations as the canonical tools so behaviour cannot drift.
//!
//! Deferred (not implemented): `background_task` (no job supervisor in this
//! binary), `browser_navigate` (no headless browser; use `fetch` /
//! `web_extract`), and the `hermes` CLI itself. Delegation is covered by the
//! existing `subagent` tool.
//!
//! Output caps: every tool that can return unbounded text truncates its
//! result to [`MODEL_OUTPUT_CHARS`] characters before handing it to the
//! model (the truncation happens inside each tool's `execute`, so the agent
//! loop needs no changes). `bash` additionally caps each stream at
//! [`BASH_OUTPUT_CAP`] bytes; `read` defaults to [`READ_DEFAULT_MAX_BYTES`]
//! bytes with a [`READ_HARD_MAX_BYTES`] ceiling.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::agent::tool::{Tool, ToolContext, ToolOutput};
use crate::error::Result;
use crate::util::truncate_bytes;

/// Per-stream byte cap for `bash` (64 KiB). Commands that flood stdout/stderr
/// past this get their reader threads disconnected so the child receives
/// SIGPIPE on the next write and exits.
const BASH_OUTPUT_CAP: usize = 64 * 1024;

/// Default `bash` timeout (seconds) when the caller passes no `timeout`.
const BASH_DEFAULT_TIMEOUT_SECS: u64 = 180;

/// Hard ceiling for `bash` timeouts (seconds); larger values are clamped.
const BASH_MAX_TIMEOUT_SECS: u64 = 600;

/// Default `read` size when `max_bytes` is omitted (bytes).
const READ_DEFAULT_MAX_BYTES: usize = 20_000;

/// Hard ceiling for `read` `max_bytes` (bytes); larger values are clamped.
const READ_HARD_MAX_BYTES: usize = 200_000;

/// Max characters of tool output forwarded to the model. Anything longer is
/// truncated with a note so one noisy tool cannot blow the context window.
pub const MODEL_OUTPUT_CHARS: usize = 30_000;

/// Truncate `s` to at most `cap` characters (char-boundary safe), appending
/// a `[truncated ...]` note when truncation occurred.
fn truncate_for_model(s: String, cap: usize) -> String {
    if s.chars().count() <= cap {
        return s;
    }
    let end = s.char_indices().nth(cap).map(|(i, _)| i).unwrap_or(s.len());
    format!(
        "{}\n[truncated to {cap} chars for model]",
        s[..end].trim_end()
    )
}

/// Truncate `s` to [`MODEL_OUTPUT_CHARS`] characters for the model.
fn cap_model_output(s: String) -> String {
    truncate_for_model(s, MODEL_OUTPUT_CHARS)
}

/// Clamp an optional caller timeout into `[1, BASH_MAX_TIMEOUT_SECS]`,
/// defaulting to [`BASH_DEFAULT_TIMEOUT_SECS`].
fn clamp_timeout(timeout: Option<u64>) -> Duration {
    Duration::from_secs(
        timeout
            .unwrap_or(BASH_DEFAULT_TIMEOUT_SECS)
            .clamp(1, BASH_MAX_TIMEOUT_SECS),
    )
}

/// Build a `/bin/sh -c <command>` pipeline honouring `ctx.cwd`.
fn shell_command(command: &str, cwd: Option<&Path>) -> std::process::Command {
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd
}

async fn run_shell(command: &str, timeout: Option<u64>, ctx: &ToolContext) -> Result<ToolOutput> {
    let cmd = shell_command(command, ctx.cwd.as_deref());
    let timeout = clamp_timeout(timeout);
    // `run_with_timeout` blocks its thread; keep it off the async runtime.
    // `shell_command` already applied `ctx.cwd`, so no cwd to re-apply here.
    let result = tokio::task::spawn_blocking(move || run_with_timeout(cmd, timeout)).await;
    match result {
        Ok(Ok(out)) => Ok(ToolOutput::ok(cap_model_output(out))),
        Ok(Err(e)) => Ok(ToolOutput::err(format!("bash failed: {e}"))),
        Err(e) => Ok(ToolOutput::err(format!("bash join error: {e}"))),
    }
}

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
                "timeout": {"type": "integer", "description": "Timeout seconds (default 180, max 600)"}
            },
            "required": ["command"]
        })
    }
    fn is_read_only(&self) -> bool {
        false
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            command: String,
            timeout: Option<u64>,
        }
        let args: Args = serde_json::from_value(input)?;
        run_shell(&args.command, args.timeout, ctx).await
    }
}

/// Hermes-compat alias for [`BashTool`] under the `execute_command` name.
pub struct ExecuteCommandTool;

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str {
        "execute_command"
    }
    fn description(&self) -> &str {
        "Run a shell command and return combined stdout+stderr. (Alias of `bash` for Hermes skills.)"
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to run"},
                "timeout": {"type": "integer", "description": "Timeout seconds (default 180, max 600)"}
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
        run_shell(&args.command, args.timeout, ctx).await
    }
}

/// Hermes-compat code runner: accepts `command` or `code` (+ `language`).
/// Python code runs via `python3 -c`; anything else runs via `/bin/sh`.
pub struct ExecuteCodeTool;

#[async_trait]
impl Tool for ExecuteCodeTool {
    fn name(&self) -> &str {
        "execute_code"
    }
    fn description(&self) -> &str {
        "Run a command or a code snippet (python via python3, else shell) and return output."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to run (alternative to code)"},
                "code": {"type": "string", "description": "Code snippet to run"},
                "language": {"type": "string", "description": "Snippet language: 'python' runs via python3, anything else via shell"},
                "timeout": {"type": "integer", "description": "Timeout seconds (default 180, max 600)"}
            }
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            command: Option<String>,
            code: Option<String>,
            language: Option<String>,
            timeout: Option<u64>,
        }
        let args: Args = serde_json::from_value(input)?;
        let timeout = clamp_timeout(args.timeout);
        if let Some(command) = args.command {
            let cmd = shell_command(&command, ctx.cwd.as_deref());
            let result = tokio::task::spawn_blocking(move || run_with_timeout(cmd, timeout)).await;
            return match result {
                Ok(Ok(out)) => Ok(ToolOutput::ok(cap_model_output(out))),
                Ok(Err(e)) => Ok(ToolOutput::err(format!("execute_code failed: {e}"))),
                Err(e) => Ok(ToolOutput::err(format!("execute_code join error: {e}"))),
            };
        }
        let Some(code) = args.code else {
            return Ok(ToolOutput::err("execute_code requires `command` or `code`"));
        };
        let is_python = args
            .language
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case("python") || l.eq_ignore_ascii_case("python3"));
        // Build the child directly (no shell) so snippet quoting cannot break.
        let mut cmd = if is_python {
            let mut c = std::process::Command::new("python3");
            c.arg("-c").arg(&code);
            c.stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .stdin(std::process::Stdio::null());
            c
        } else {
            shell_command(&code, None)
        };
        if let Some(cwd) = &ctx.cwd {
            cmd.current_dir(cwd);
        }
        let result = tokio::task::spawn_blocking(move || run_with_timeout(cmd, timeout)).await;
        match result {
            Ok(Ok(out)) => Ok(ToolOutput::ok(cap_model_output(out))),
            Ok(Err(e)) => Ok(ToolOutput::err(format!("execute_code failed: {e}"))),
            Err(e) => Ok(ToolOutput::err(format!("execute_code join error: {e}"))),
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

    let (mut out_buf, out_truncated) = t_out.join().unwrap_or_default();
    let (err_buf, err_truncated) = t_err.join().unwrap_or_default();
    // Enforce the per-stream cap on bytes actually kept: `read_capped`
    // over-reads by one byte to detect the cap, so trim that probe byte.
    out_buf.truncate(BASH_OUTPUT_CAP);
    let mut out = String::from_utf8_lossy(&out_buf).into_owned();
    let err_full = String::from_utf8_lossy(&err_buf);
    // `read_capped` over-reads by one probe byte; re-cap char-safe.
    let err: String = err_full.chars().take(BASH_OUTPUT_CAP).collect();
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

/// Read a file, honouring an explicit `max_bytes`, the
/// [`READ_DEFAULT_MAX_BYTES`] default, and the [`READ_HARD_MAX_BYTES`]
/// ceiling. Shared by `read` and `read_file`.
async fn read_file_impl(path: &Path, max_bytes: Option<usize>) -> Result<ToolOutput> {
    let max = max_bytes
        .unwrap_or(READ_DEFAULT_MAX_BYTES)
        .min(READ_HARD_MAX_BYTES);
    let mut buf = tokio::fs::read(path)
        .await
        .map_err(|e| anyhow::anyhow!("read {} failed: {e}", path.display()))?;
    let truncated = buf.len() > max;
    buf.truncate(max);
    let mut content = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        content.push_str(&format!(
            "\n[truncated to {max} bytes; pass max_bytes to read more]"
        ));
    }
    Ok(ToolOutput::ok(cap_model_output(content)))
}

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
                "max_bytes": {"type": "integer", "description": "Truncate to this many bytes (default 20000, max 200000)"}
            },
            "required": ["path"]
        })
    }
    fn is_read_only(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            max_bytes: Option<usize>,
        }
        let args: Args = serde_json::from_value(input)?;
        let path = resolve_path(&args.path, ctx.cwd.as_deref());
        read_file_impl(&path, args.max_bytes).await
    }
}

/// Hermes-compat alias for [`ReadTool`] under the `read_file` name.
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read a file from disk and return its contents. (Alias of `read` for Hermes skills.)"
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Absolute or cwd-relative path"},
                "max_bytes": {"type": "integer", "description": "Truncate to this many bytes (default 20000, max 200000)"}
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
        read_file_impl(&path, args.max_bytes).await
    }
}

// WriteTool

/// Write `content` to `path`, creating parent directories as needed.
/// Shared by `write` and `write_file`.
async fn write_file_impl(path: &Path, content: &str) -> Result<ToolOutput> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let bytes = content.len();
    tokio::fs::write(path, content.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write {} failed: {e}", path.display()))?;
    Ok(ToolOutput::ok(format!(
        "wrote {bytes} bytes to {}",
        path.display()
    )))
}

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
    fn is_read_only(&self) -> bool {
        false
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            content: String,
        }
        let args: Args = serde_json::from_value(input)?;
        let path = resolve_path(&args.path, ctx.cwd.as_deref());
        write_file_impl(&path, &args.content).await
    }
}

/// Hermes-compat alias for [`WriteTool`] under the `write_file` name.
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Write a file to disk, creating parent dirs if needed. (Alias of `write` for Hermes skills.)"
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
        write_file_impl(&path, &args.content).await
    }
}

/// Patch a file by replacing one exact string. Fails when `old_str` is
/// absent or ambiguous. Shared by `edit` (arbitrary files) and the `skill`
/// `patch` action (which adds a frontmatter check on top).
async fn edit_file_impl(
    path: &Path,
    old_str: &str,
    new_str: &str,
    replace_all: bool,
) -> std::result::Result<String, String> {
    if old_str.is_empty() {
        return Err("old_str must not be empty".to_string());
    }
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("read {} failed: {e}", path.display()))?;
    let matches = content.matches(old_str).count();
    if matches == 0 {
        return Err("old_str not found in file".to_string());
    }
    if matches > 1 && !replace_all {
        return Err(format!(
            "old_str matches {matches} times; pass replace_all=true to replace all, or narrow old_str"
        ));
    }
    let updated = if replace_all {
        content.replace(old_str, new_str)
    } else {
        content.replacen(old_str, new_str, 1)
    };
    tokio::fs::write(path, updated.as_bytes())
        .await
        .map_err(|e| format!("write {} failed: {e}", path.display()))?;
    Ok(if replace_all {
        format!("replaced {matches} occurrence(s) in {}", path.display())
    } else {
        format!("patched {}", path.display())
    })
}

/// Patch a file by replacing an exact string (single match by default).
pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "Replace an exact string in a file. Fails when old_str is missing or matches more than once (unless replace_all is true)."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Absolute or cwd-relative path"},
                "old_str": {"type": "string", "description": "Exact string to replace (must occur once)"},
                "new_str": {"type": "string", "description": "Replacement string"},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence (default false)"}
            },
            "required": ["path", "old_str", "new_str"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            old_str: String,
            new_str: String,
            replace_all: Option<bool>,
        }
        let args: Args = serde_json::from_value(input)?;
        let path = resolve_path(&args.path, ctx.cwd.as_deref());
        match edit_file_impl(
            &path,
            &args.old_str,
            &args.new_str,
            args.replace_all.unwrap_or(false),
        )
        .await
        {
            Ok(msg) => Ok(ToolOutput::ok(msg)),
            Err(e) => Ok(ToolOutput::err(e)),
        }
    }
}

// SearchFilesTool

/// Max files `search_files` will inspect per call (bounds huge trees).
const SEARCH_MAX_FILES: usize = 10_000;

/// Max file size `search_files` will read (larger files are skipped).
const SEARCH_MAX_FILE_BYTES: u64 = 512 * 1024;

/// Recursively grep `pattern` (substring match) under `dir`, pushing
/// `path:line: text` hits into `out`. Stops early once `out` reaches
/// `max_results`. Returns `(files_visited, stopped_early)`.
fn search_dir(
    dir: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    max_results: usize,
    files_visited: &mut usize,
) {
    if out.len() >= max_results || *files_visited > SEARCH_MAX_FILES {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    // Sort for deterministic output.
    let mut entries: Vec<_> = entries.filter_map(std::result::Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= max_results || *files_visited > SEARCH_MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        // Never follow symlinks: avoids cycles and escaping the tree.
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            // Skip dependency / VCS noise that dwarfs real hits.
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(
                    name,
                    ".git" | "target" | "node_modules" | ".venv" | "__pycache__"
                ) {
                    continue;
                }
            }
            search_dir(&path, pattern, out, max_results, files_visited);
        } else if ft.is_file() {
            *files_visited += 1;
            if let Ok(meta) = std::fs::metadata(&path) {
                if meta.len() > SEARCH_MAX_FILE_BYTES {
                    continue;
                }
            }
            let Ok(raw) = std::fs::read(&path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&raw);
            for (i, line) in text.lines().enumerate() {
                if line.contains(pattern) {
                    out.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        i + 1,
                        truncate_bytes(line.trim(), 300)
                    ));
                    if out.len() >= max_results {
                        return;
                    }
                }
            }
        }
    }
}

/// Recursive substring search over file contents (std only, no walker dep).
pub struct SearchFilesTool;

#[async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }
    fn description(&self) -> &str {
        "Recursively search file contents for a substring. Returns path:line hits."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Substring to search for"},
                "dir": {"type": "string", "description": "Directory to search (default: cwd)"},
                "max_results": {"type": "integer", "description": "Max hits to return (default 20, max 100)"}
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            pattern: String,
            dir: Option<String>,
            max_results: Option<usize>,
        }
        let args: Args = serde_json::from_value(input)?;
        if args.pattern.is_empty() {
            return Ok(ToolOutput::err(
                "search_files requires a non-empty `pattern`",
            ));
        }
        let max_results = args.max_results.unwrap_or(20).clamp(1, 100);
        let dir = match &args.dir {
            Some(d) => resolve_path(d, ctx.cwd.as_deref()),
            None => ctx.cwd.clone().unwrap_or_else(|| PathBuf::from(".")),
        };
        // `search_dir` is blocking IO; keep it off the async runtime.
        let pattern = args.pattern.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let mut visited = 0usize;
            search_dir(&dir, &pattern, &mut out, max_results, &mut visited);
            (out, visited)
        })
        .await;
        match result {
            Ok((hits, visited)) => {
                if hits.is_empty() {
                    Ok(ToolOutput::ok(format!(
                        "no matches for {:?} ({visited} files searched)",
                        args.pattern
                    )))
                } else {
                    Ok(ToolOutput::ok(cap_model_output(hits.join("\n"))))
                }
            }
            Err(e) => Ok(ToolOutput::err(format!("search join error: {e}"))),
        }
    }
}

// FetchTool + web helpers

/// Hard cap on fetched response size (50 KiB). Bodies larger than this
/// are truncated and the rest of the stream is dropped so a hostile or
/// huge URL can't OOM the process.
const FETCH_BODY_CAP: usize = 50 * 1024;

/// Build the shared HTTP client (20s timeout). Falls back to a default
/// client instead of panicking — the release profile sets `panic = "abort"`.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "http client build failed; using default client");
            reqwest::Client::new()
        })
}

/// Strip `<script>`/`<style>` blocks, then tags, then common entities, and
/// collapse whitespace. Best-effort HTML-to-text for `web_extract` /
/// `web_search` — not a full renderer.
fn strip_html(html: &str) -> String {
    let no_script = remove_tag_block(html, "script");
    let no_style = remove_tag_block(&no_script, "style");
    let mut out = String::with_capacity(no_style.len().min(32 * 1024));
    let mut in_tag = false;
    for c in no_style.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse runs of blank lines and trailing whitespace per line.
    let mut collapsed = String::with_capacity(decoded.len());
    for line in decoded.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            if !collapsed.ends_with("\n\n") {
                collapsed.push('\n');
            }
            continue;
        }
        collapsed.push_str(line);
        collapsed.push('\n');
    }
    collapsed.trim().to_string()
}

/// Remove `<tag ...>...</tag>` blocks case-insensitively.
fn remove_tag_block(html: &str, tag: &str) -> String {
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(html.len());
    let mut rest = 0usize;
    while let Some(start_rel) = lower[rest..].find(open.as_str()) {
        let start = rest + start_rel;
        let Some(end_rel) = lower[start..].find(close.as_str()) else {
            break;
        };
        out.push_str(&html[rest..start]);
        rest = start + end_rel + close.len();
    }
    out.push_str(&html[rest..]);
    out
}

/// Percent-encode a query string (unreserved chars pass through).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Fetch a URL (HTTP GET) and return the body.
pub struct FetchTool {
    client: reqwest::Client,
}

impl Default for FetchTool {
    fn default() -> Self {
        Self {
            client: http_client(),
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
    fn is_read_only(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            url: String,
        }
        let args: Args = serde_json::from_value(input)?;
        let body = fetch_body_capped(&self.client, &args.url).await?;
        Ok(ToolOutput::ok(cap_model_output(body)))
    }
}

/// GET `url`, streaming the body up to [`FETCH_BODY_CAP`] bytes.
/// Returns `"[status]\nbody"` with a truncation note when capped.
async fn fetch_body_capped(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
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
    let body = if truncated {
        format!(
            "{}\n[...truncated at {} bytes]",
            String::from_utf8_lossy(&buf),
            FETCH_BODY_CAP
        )
    } else {
        String::from_utf8_lossy(&buf).into_owned()
    };
    Ok(format!("[{status}]\n{body}"))
}

/// Hermes-compat web search via DuckDuckGo Lite (best-effort text extract).
/// Requires network; without it the tool reports the failure so the model
/// can fall back to `fetch` with an explicit URL.
pub struct WebSearchTool {
    client: reqwest::Client,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self {
            client: http_client(),
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Search the web (DuckDuckGo Lite) and return text results. Falls back to `fetch` with a direct URL when offline."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {"query": {"type": "string", "description": "Search query"}},
            "required": ["query"]
        })
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            query: String,
        }
        let args: Args = serde_json::from_value(input)?;
        if args.query.trim().is_empty() {
            return Ok(ToolOutput::err("web_search requires a non-empty `query`"));
        }
        let url = format!(
            "https://lite.duckduckgo.com/lite/?q={}",
            url_encode(&args.query)
        );
        match fetch_body_capped(&self.client, &url).await {
            Ok(body) => {
                // Strip the status line before de-HTML-ing so "[200 OK]"
                // doesn't pollute the text.
                let text = strip_html(body.split_once('\n').map(|(_, b)| b).unwrap_or(""));
                if text.trim().is_empty() {
                    Ok(ToolOutput::err(
                        "web_search returned no readable text; try `fetch` with a direct URL",
                    ))
                } else {
                    Ok(ToolOutput::ok(cap_model_output(text)))
                }
            }
            Err(e) => Ok(ToolOutput::err(format!(
                "web_search failed ({e}); try `fetch` with a direct URL"
            ))),
        }
    }
}

/// Hermes-compat readable-page fetch: like `fetch`, but de-HTML-ed.
pub struct WebExtractTool {
    client: reqwest::Client,
}

impl Default for WebExtractTool {
    fn default() -> Self {
        Self {
            client: http_client(),
        }
    }
}

#[async_trait]
impl Tool for WebExtractTool {
    fn name(&self) -> &str {
        "web_extract"
    }
    fn description(&self) -> &str {
        "Fetch a URL and return its readable text (HTML tags stripped)."
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
        if args.url.trim().is_empty() {
            return Ok(ToolOutput::err("web_extract requires a non-empty `url`"));
        }
        match fetch_body_capped(&self.client, &args.url).await {
            Ok(body) => {
                let text = strip_html(body.split_once('\n').map(|(_, b)| b).unwrap_or(""));
                if text.trim().is_empty() {
                    Ok(ToolOutput::err(
                        "web_extract found no readable text at that URL",
                    ))
                } else {
                    Ok(ToolOutput::ok(cap_model_output(text)))
                }
            }
            Err(e) => Ok(ToolOutput::err(format!("web_extract failed: {e}"))),
        }
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
        "Durable memory: read, write, list, delete, or search notes and past conversation. \
         Proactively write compact declarative facts the user states (stable preferences, \
         environment details, project conventions) without being asked; never store secrets \
         or ephemeral task state. action='read'|'write'|'list'|'delete'|'search'."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["read", "write", "list", "delete", "search"]},
                "key": {"type": "string", "description": "Note key (read/write/delete); fallback query for search"},
                "value": {"type": "string", "description": "Note value (write only)"},
                "query": {"type": "string", "description": "Search query (search only; falls back to `key`)"}
            },
            "required": ["action"]
        })
    }
    /// Conservative: `read`/`list`/`search` are side-effect free, but
    /// `write`/`delete` mutate. The trait only offers a static flag, so
    /// report `false` and run serially rather than risk concurrent writes.
    fn is_read_only(&self) -> bool {
        false
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            key: Option<String>,
            value: Option<String>,
            query: Option<String>,
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
            "read" => Ok(ToolOutput::ok(cap_model_output(
                memory
                    .read_note(key()?)?
                    .unwrap_or_else(|| "(no note)".to_string()),
            ))),
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
                Ok(ToolOutput::ok(cap_model_output(body)))
            }
            "search" => {
                let query = args
                    .query
                    .as_deref()
                    .or(args.key.as_deref())
                    .ok_or_else(|| anyhow::anyhow!("memory.search requires `query` (or `key`)"))?;
                let hits = memory.search_history_excluding(query, 8, ctx.session_id.as_deref())?;
                Ok(ToolOutput::ok(cap_model_output(
                    hits.into_iter()
                        .map(|hit| {
                            format!("- [{}] {}", hit.session_id, truncate_bytes(&hit.text, 500))
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                )))
            }
            "delete" => {
                memory.delete_note(key()?)?;
                Ok(ToolOutput::ok("ok".to_string()))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

// SkillTool

/// Reject path traversal / empty names for skill file operations.
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

/// Resolve a skill `name` to its `SKILL.md` path via the registry, so nested
/// category layouts (e.g. `skills/devops/foo/SKILL.md`) work — not just the
/// flat `root.join(name)` layout.
fn resolve_skill_path(root: &Path, name: &str) -> Option<PathBuf> {
    crate::skills::SkillRegistry::load_dir(root)
        .ok()?
        .get(name)
        .map(|s| s.path.clone())
}

/// Validate `SKILL.md` content: must start with `---` frontmatter containing
/// non-empty `name:` and `description:` entries.
fn validate_skill_content(content: &str) -> Result<()> {
    let (front, _) = crate::skills::split_frontmatter(content);
    if front.trim().is_empty() {
        anyhow::bail!("skill content must start with --- frontmatter");
    }
    let has = |key: &str| {
        front.lines().any(|l| {
            l.trim()
                .strip_prefix(key)
                .is_some_and(|v| !v.trim().trim_matches(['"', '\'']).is_empty())
        })
    };
    if !has("name:") || !has("description:") {
        anyhow::bail!("skill frontmatter must contain non-empty 'name:' and 'description:'");
    }
    Ok(())
}

/// Read a skill's `SKILL.md` body by registry name.
async fn read_skill_by_name(root: &Path, name: &str) -> Result<ToolOutput> {
    let Some(path) = resolve_skill_path(root, name) else {
        return Ok(ToolOutput::err(format!("unknown skill: {name}")));
    };
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| anyhow::anyhow!("read skill {name} failed: {e}"))?;
    Ok(ToolOutput::ok(cap_model_output(content)))
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
        "List, read, create, or patch reusable SKILL.md procedures."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object", "properties": {
                "action": {"type": "string", "enum": ["list", "read", "write", "patch"]},
                "name": {"type": "string", "description": "skill name (letters, digits, _, -)"},
                "content": {"type": "string", "description": "full SKILL.md content for write (must include --- frontmatter with name: and description:)"},
                "old_str": {"type": "string", "description": "exact string to replace for patch (must occur once)"},
                "new_str": {"type": "string", "description": "replacement string for patch"}
            }, "required": ["action"]
        })
    }
    /// Conservative: `list`/`read` are side-effect free, but `write`
    /// creates files. The trait only offers a static flag, so report
    /// `false` and run serially rather than risk concurrent writes.
    fn is_read_only(&self) -> bool {
        false
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            name: Option<String>,
            content: Option<String>,
            old_str: Option<String>,
            new_str: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        match args.action.as_str() {
            "list" => Ok(ToolOutput::ok(cap_model_output(
                crate::skills::SkillRegistry::load_dir(&self.root)?
                    .iter()
                    .map(|s| s.index_line())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ))),
            "read" => {
                let name = safe_skill_name(args.name.as_deref())?;
                read_skill_by_name(&self.root, &name).await
            }
            "write" => {
                let name = safe_skill_name(args.name.as_deref())?;
                let content = args
                    .content
                    .ok_or_else(|| anyhow::anyhow!("skill.write requires content"))?;
                if let Err(e) = validate_skill_content(&content) {
                    return Ok(ToolOutput::err(format!("invalid SKILL.md: {e}")));
                }
                let path = self.root.join(&name).join("SKILL.md");
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| anyhow::anyhow!("create skill dir failed: {e}"))?;
                }
                tokio::fs::write(&path, content)
                    .await
                    .map_err(|e| anyhow::anyhow!("write skill failed: {e}"))?;
                Ok(ToolOutput::ok(format!("saved learned skill {name}")))
            }
            "patch" => {
                let name = safe_skill_name(args.name.as_deref())?;
                let (Some(old_str), Some(new_str)) =
                    (args.old_str.as_deref(), args.new_str.as_deref())
                else {
                    return Ok(ToolOutput::err(
                        "skill.patch requires `old_str` and `new_str`",
                    ));
                };
                let Some(path) = resolve_skill_path(&self.root, &name) else {
                    return Ok(ToolOutput::err(format!("unknown skill: {name}")));
                };
                match edit_file_impl(&path, old_str, new_str, false).await {
                    Ok(_) => {
                        // The patch must not destroy the frontmatter header.
                        let updated = tokio::fs::read_to_string(&path)
                            .await
                            .map_err(|e| anyhow::anyhow!("re-read skill failed: {e}"))?;
                        if let Err(e) = validate_skill_content(&updated) {
                            return Ok(ToolOutput::err(format!(
                                "patch would break SKILL.md header ({e}); file left patched — rewrite with `write` to repair"
                            )));
                        }
                        Ok(ToolOutput::ok(format!("patched skill {name}")))
                    }
                    Err(e) => Ok(ToolOutput::err(e)),
                }
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

/// Hermes-compat alias for reading skills under the `skill_view` name.
pub struct SkillViewTool {
    root: PathBuf,
}

impl SkillViewTool {
    /// Create a skill-view tool rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl Tool for SkillViewTool {
    fn name(&self) -> &str {
        "skill_view"
    }
    fn description(&self) -> &str {
        "Read a SKILL.md procedure by name. (Alias of `skill` action=read for Hermes skills.)"
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "skill name"}
            },
            "required": ["name"]
        })
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            name: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        let name = safe_skill_name(args.name.as_deref())?;
        read_skill_by_name(&self.root, &name).await
    }
}

// TodoTool

/// Memory note key backing the [`TodoTool`] list.
const TODOS_NOTE_KEY: &str = "todos";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TodoEntry {
    text: String,
    done: bool,
}

fn load_todos(memory: &crate::memory::Memory) -> Result<Vec<TodoEntry>> {
    match memory.read_note(TODOS_NOTE_KEY)? {
        None => Ok(Vec::new()),
        Some(raw) if raw.trim().is_empty() => Ok(Vec::new()),
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("todos note is corrupt ({e}); use action=clear to reset")),
    }
}

fn render_todos(todos: &[TodoEntry]) -> String {
    if todos.is_empty() {
        return "(no todos)".to_string();
    }
    todos
        .iter()
        .enumerate()
        .map(|(i, t)| {
            format!(
                "- [{}] {}. {}",
                if t.done { "x" } else { " " },
                i + 1,
                t.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Track a small task list in memory (`todos` note). Wave D's gateway may
/// surface this; for now it is durable per-memory-backend state.
pub struct TodoTool;

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }
    fn description(&self) -> &str {
        "Track tasks: list, add, done, clear. Stored in the memory `todos` note."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "add", "done", "clear"]},
                "text": {"type": "string", "description": "Task text (add only)"},
                "index": {"type": "integer", "description": "1-based task number (done only)"}
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            action: String,
            text: Option<String>,
            index: Option<usize>,
        }
        let args: Args = serde_json::from_value(input)?;
        let Some(memory) = &ctx.memory else {
            return Ok(ToolOutput::err(
                "memory backend not configured in this session",
            ));
        };
        match args.action.as_str() {
            "list" => Ok(ToolOutput::ok(cap_model_output(render_todos(&load_todos(
                memory,
            )?)))),
            "add" => {
                let Some(text) = args
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                else {
                    return Ok(ToolOutput::err("todo.add requires non-empty `text`"));
                };
                let mut todos = load_todos(memory)?;
                todos.push(TodoEntry {
                    text: text.to_string(),
                    done: false,
                });
                let n = todos.len();
                memory.write_note(
                    TODOS_NOTE_KEY,
                    &serde_json::to_string(&todos).map_err(|e| anyhow::anyhow!("{e}"))?,
                )?;
                Ok(ToolOutput::ok(format!("added #{n}: {text}")))
            }
            "done" => {
                let Some(index) = args.index else {
                    return Ok(ToolOutput::err("todo.done requires 1-based `index`"));
                };
                let mut todos = load_todos(memory)?;
                if index == 0 || index > todos.len() {
                    return Ok(ToolOutput::err(format!(
                        "todo #{index} does not exist ({} todos)",
                        todos.len()
                    )));
                }
                todos[index - 1].done = true;
                let text = todos[index - 1].text.clone();
                memory.write_note(
                    TODOS_NOTE_KEY,
                    &serde_json::to_string(&todos).map_err(|e| anyhow::anyhow!("{e}"))?,
                )?;
                Ok(ToolOutput::ok(format!("done #{index}: {text}")))
            }
            "clear" => {
                memory.delete_note(TODOS_NOTE_KEY)?;
                Ok(ToolOutput::ok("cleared todos".to_string()))
            }
            other => Ok(ToolOutput::err(format!("unknown action: {other}"))),
        }
    }
}

// AskTool

/// UI-mediated question helper. Non-interactive surfaces (gateway, web, run)
/// cannot prompt mid-turn, so this echoes the question back: the model must
/// present the options in chat text and proceed with the most reasonable
/// default when the user does not answer.
pub struct AskTool;

#[async_trait]
impl Tool for AskTool {
    fn name(&self) -> &str {
        "ask"
    }
    fn description(&self) -> &str {
        "Pose a question with options. UI-mediated: present the options in chat text and proceed with a reasonable default."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "Question to put to the user"},
                "options": {"type": "array", "items": {"type": "string"}, "description": "Candidate answers"}
            },
            "required": ["question"]
        })
    }
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Deserialize)]
        struct Args {
            question: String,
            options: Option<Vec<String>>,
        }
        let args: Args = serde_json::from_value(input)?;
        if args.question.trim().is_empty() {
            return Ok(ToolOutput::err("ask requires a non-empty `question`"));
        }
        let mut out = format!("Question for user: {}", args.question.trim());
        if let Some(options) = args.options.filter(|o| !o.is_empty()) {
            out.push_str("\nOptions:");
            for (i, opt) in options.iter().enumerate() {
                out.push_str(&format!("\n  {}. {}", i + 1, opt));
            }
        }
        out.push_str(
            "\n(ask is UI-mediated: present the options in chat text and proceed with the most reasonable default.)",
        );
        Ok(ToolOutput::ok(out))
    }
}

// MessageTool

/// Queue an outbound message in the memory outbox (`outbox:<session>`).
/// Wave D's gateway is expected to drain this after each turn and deliver
/// the entries; until then the tool is honest durable queueing, not delivery.
pub struct MessageTool;

#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str {
        "message"
    }
    fn description(&self) -> &str {
        "Queue an outbound message in the memory outbox for delivery (drained by the gateway)."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Message text to send"},
                "channel": {"type": "string", "description": "Destination channel (default: current session channel)"},
                "chat_id": {"type": "string", "description": "Destination chat id (default: current session)"}
            },
            "required": ["text"]
        })
    }
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct OutboxEntry {
            channel: Option<String>,
            chat_id: Option<String>,
            text: String,
        }
        #[derive(Deserialize)]
        struct Args {
            text: String,
            channel: Option<String>,
            chat_id: Option<String>,
        }
        let args: Args = serde_json::from_value(input)?;
        if args.text.trim().is_empty() {
            return Ok(ToolOutput::err("message requires non-empty `text`"));
        }
        let Some(memory) = &ctx.memory else {
            return Ok(ToolOutput::err(
                "memory backend not configured in this session",
            ));
        };
        // TODO(Wave D): gateway drains `outbox:<session>` after each turn.
        let session = ctx.session_id.as_deref().unwrap_or("default");
        let key = format!("outbox:{session}");
        let mut entries: Vec<OutboxEntry> = match memory.read_note(&key)? {
            None => Vec::new(),
            Some(raw) if raw.trim().is_empty() => Vec::new(),
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|e| anyhow::anyhow!("outbox note is corrupt: {e}"))?,
        };
        entries.push(OutboxEntry {
            channel: args.channel,
            chat_id: args.chat_id,
            text: args.text,
        });
        let n = entries.len();
        memory.write_note(
            &key,
            &serde_json::to_string(&entries).map_err(|e| anyhow::anyhow!("{e}"))?,
        )?;
        Ok(ToolOutput::ok(format!("queued message #{n} in {key}")))
    }
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
    /// Sub-agents inherit the parent's tools (including `bash`/`write`),
    /// so they can mutate. Always run serially.
    fn is_read_only(&self) -> bool {
        false
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
        Ok(ToolOutput::ok(cap_model_output(results.join("\n\n"))))
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

    #[test]
    fn bash_schema_timeout_defaults() {
        let s = BashTool.schema();
        assert!(
            s["properties"]["timeout"]["description"]
                .as_str()
                .unwrap_or("")
                .contains("180"),
            "schema must advertise the 180s default"
        );
    }

    #[test]
    fn clamp_timeout_defaults_and_caps() {
        assert_eq!(clamp_timeout(None), Duration::from_secs(180));
        assert_eq!(clamp_timeout(Some(6000)), Duration::from_secs(600));
        assert_eq!(clamp_timeout(Some(5)), Duration::from_secs(5));
    }

    #[test]
    fn model_output_cap_truncates_with_note() {
        let big = "x".repeat(MODEL_OUTPUT_CHARS + 10);
        let out = cap_model_output(big);
        assert!(out.contains("[truncated to 30000 chars for model]"));
        assert!(out.chars().count() > MODEL_OUTPUT_CHARS);
        assert!(out.chars().count() < MODEL_OUTPUT_CHARS + 100);
        assert_eq!(cap_model_output("small".to_string()), "small");
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

    #[tokio::test]
    async fn read_default_cap_truncates_with_note() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("big.txt");
        tokio::fs::write(&path, "y".repeat(READ_DEFAULT_MAX_BYTES + 100))
            .await
            .unwrap();
        let ctx = ToolContext {
            cwd: Some(tmp.path().to_path_buf()),
            ..Default::default()
        };
        let out = ReadTool
            .execute(json!({"path": "big.txt"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("[truncated to 20000 bytes"));
        // Explicit max_bytes is honoured (and capped at the hard max).
        let out = ReadTool
            .execute(json!({"path": "big.txt", "max_bytes": 10}), &ctx)
            .await
            .unwrap();
        assert!(out.content.starts_with("yyyyyyyyyy"));
        assert!(out.content.contains("[truncated to 10 bytes"));
        let out = ReadTool
            .execute(json!({"path": "big.txt", "max_bytes": 999_999_999}), &ctx)
            .await
            .unwrap();
        assert!(!out.content.contains("999999999"));
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

    #[tokio::test]
    async fn memory_search_accepts_query_field() {
        let mem = Arc::new(crate::memory::Memory::in_memory().unwrap());
        let ctx = ToolContext {
            memory: Some(mem),
            ..Default::default()
        };
        let tool = MemoryTool;
        // `search` with the explicit `query` field (no `key`).
        let out = tool
            .execute(json!({"action": "search", "query": "nothing-here"}), &ctx)
            .await
            .unwrap();
        assert!(!out.is_error);
        // Missing both `query` and `key` is a caller error.
        let err = tool
            .execute(json!({"action": "search"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("query"));
    }

    #[test]
    fn memory_schema_lists_all_five_actions() {
        let s = MemoryTool.schema();
        let actions = s["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>();
        for a in ["read", "write", "list", "delete", "search"] {
            assert!(actions.contains(&a), "schema missing action {a}");
        }
        assert!(s["properties"]["query"].is_object());
        assert!(MemoryTool.description().contains("search"));
    }

    fn write_skill_tree(root: &Path, rel: &str, raw: &str) {
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), raw).unwrap();
    }

    const SKILL_RAW: &str = "---\nname: nested-skill\ndescription: nested test\n---\n# Body\n";

    #[tokio::test]
    async fn skill_read_resolves_nested_layout() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill_tree(tmp.path(), "category/nested-skill", SKILL_RAW);
        let tool = SkillTool::new(tmp.path().to_path_buf());
        let out = tool
            .execute(
                json!({"action": "read", "name": "nested-skill"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("# Body"));
        // Unknown names report a tool error, not a transport error.
        let out = tool
            .execute(
                json!({"action": "read", "name": "missing"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
        // skill_view reads through the same registry path.
        let view = SkillViewTool::new(tmp.path().to_path_buf());
        let out = view
            .execute(json!({"name": "nested-skill"}), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.content.contains("# Body"));
    }

    #[tokio::test]
    async fn skill_write_validates_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = SkillTool::new(tmp.path().to_path_buf());
        let out = tool
            .execute(
                json!({"action": "write", "name": "plain", "content": "no header here"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error, "headerless write must be rejected");
        let out = tool
            .execute(
                json!({"action": "write", "name": "nn", "content": "---\nname: nn\n---\nbody"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error, "missing description must be rejected");
        let out = tool
            .execute(
                json!({"action": "write", "name": "good", "content": "---\nname: good\ndescription: d\n---\nbody"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
    }

    #[tokio::test]
    async fn skill_patch_replaces_once() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill_tree(
            tmp.path(),
            "good",
            "---\nname: good\ndescription: d\n---\nhello world\n",
        );
        let tool = SkillTool::new(tmp.path().to_path_buf());
        let out = tool
            .execute(
                json!({"action": "patch", "name": "good", "old_str": "hello", "new_str": "bye"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error, "{}", out.content);
        let out = tool
            .execute(
                json!({"action": "read", "name": "good"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("bye world"));
        // Missing old_str is a tool error.
        let out = tool
            .execute(
                json!({"action": "patch", "name": "good", "old_str": "nope", "new_str": "x"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn skill_patch_rejects_ambiguous_match() {
        let tmp = tempfile::tempdir().unwrap();
        write_skill_tree(
            tmp.path(),
            "dup",
            "---\nname: dup\ndescription: d\n---\na a a\n",
        );
        let tool = SkillTool::new(tmp.path().to_path_buf());
        let out = tool
            .execute(
                json!({"action": "patch", "name": "dup", "old_str": "a", "new_str": "b"}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
        assert!(out.content.contains("times"));
    }

    #[tokio::test]
    async fn edit_tool_single_and_multi() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            cwd: Some(tmp.path().to_path_buf()),
            ..Default::default()
        };
        tokio::fs::write(tmp.path().join("f.txt"), "alpha beta alpha\n")
            .await
            .unwrap();
        let tool = EditTool;
        // Ambiguous without replace_all.
        let out = tool
            .execute(
                json!({"path": "f.txt", "old_str": "alpha", "new_str": "gamma"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.is_error);
        // Missing string.
        let out = tool
            .execute(
                json!({"path": "f.txt", "old_str": "zzz", "new_str": "gamma"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.is_error);
        // Single unambiguous replacement.
        let out = tool
            .execute(
                json!({"path": "f.txt", "old_str": "beta", "new_str": "GAMMA"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.is_error, "{}", out.content);
        assert_eq!(
            tokio::fs::read_to_string(tmp.path().join("f.txt"))
                .await
                .unwrap(),
            "alpha GAMMA alpha\n"
        );
    }

    #[tokio::test]
    async fn search_files_finds_content() {
        let tmp = tempfile::tempdir().unwrap();
        tokio::fs::write(
            tmp.path().join("a.txt"),
            "hello needle world\nsecond line\n",
        )
        .await
        .unwrap();
        tokio::fs::create_dir_all(tmp.path().join("sub"))
            .await
            .unwrap();
        tokio::fs::write(tmp.path().join("sub/b.txt"), "no match\n")
            .await
            .unwrap();
        let ctx = ToolContext {
            cwd: Some(tmp.path().to_path_buf()),
            ..Default::default()
        };
        let out = SearchFilesTool
            .execute(json!({"pattern": "needle"}), &ctx)
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("a.txt:1:"));
        let out = SearchFilesTool
            .execute(json!({"pattern": "absent-xyz"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("no matches"));
        let out = SearchFilesTool
            .execute(json!({"pattern": ""}), &ctx)
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn todo_tool_round_trip() {
        let mem = Arc::new(crate::memory::Memory::in_memory().unwrap());
        let ctx = ToolContext {
            memory: Some(mem),
            ..Default::default()
        };
        let out = TodoTool
            .execute(json!({"action": "add", "text": "write tests"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("#1"));
        let out = TodoTool
            .execute(json!({"action": "list"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("write tests"));
        let out = TodoTool
            .execute(json!({"action": "done", "index": 1}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("done #1"));
        let out = TodoTool
            .execute(json!({"action": "done", "index": 9}), &ctx)
            .await
            .unwrap();
        assert!(out.is_error);
        let out = TodoTool
            .execute(json!({"action": "clear"}), &ctx)
            .await
            .unwrap();
        assert!(!out.is_error);
        let out = TodoTool
            .execute(json!({"action": "list"}), &ctx)
            .await
            .unwrap();
        assert!(out.content.contains("(no todos)"));
    }

    #[tokio::test]
    async fn ask_tool_echoes_question() {
        let out = AskTool
            .execute(
                json!({"question": "Pick one?", "options": ["a", "b"]}),
                &ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content.contains("Pick one?"));
        assert!(out.content.contains("a"));
        let out = AskTool
            .execute(json!({"question": "  "}), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn message_tool_queues_outbox() {
        let mem = Arc::new(crate::memory::Memory::in_memory().unwrap());
        let ctx = ToolContext {
            memory: Some(mem.clone()),
            session_id: Some("s1".to_string()),
            ..Default::default()
        };
        let out = MessageTool
            .execute(json!({"text": "hi there"}), &ctx)
            .await
            .unwrap();
        assert!(!out.is_error, "{}", out.content);
        assert!(out.content.contains("outbox:s1"));
        let raw = mem.read_note("outbox:s1").unwrap().unwrap();
        assert!(raw.contains("hi there"));
    }

    #[test]
    fn strip_html_removes_tags_and_scripts() {
        let html = "<html><head><script>var x=1;</script></head><body><h1>Hi &amp; bye</h1><p>Text</p></body></html>";
        let text = strip_html(html);
        assert!(!text.contains("var x=1"));
        assert!(!text.contains('<'));
        assert!(text.contains("Hi & bye"));
        assert!(text.contains("Text"));
    }

    #[test]
    fn url_encode_escapes_query() {
        assert_eq!(url_encode("a b&c"), "a+b%26c");
    }

    #[test]
    fn compat_aliases_have_distinct_names() {
        for (tool, name) in [
            (&BashTool as &dyn Tool, "bash"),
            (&ExecuteCommandTool as &dyn Tool, "execute_command"),
            (&ExecuteCodeTool as &dyn Tool, "execute_code"),
            (&ReadTool as &dyn Tool, "read"),
            (&ReadFileTool as &dyn Tool, "read_file"),
            (&WriteTool as &dyn Tool, "write"),
            (&WriteFileTool as &dyn Tool, "write_file"),
            (&SearchFilesTool as &dyn Tool, "search_files"),
            (
                &SkillViewTool::new(PathBuf::from("/x")) as &dyn Tool,
                "skill_view",
            ),
            (&WebSearchTool::default() as &dyn Tool, "web_search"),
            (&WebExtractTool::default() as &dyn Tool, "web_extract"),
            (&EditTool as &dyn Tool, "edit"),
            (&TodoTool as &dyn Tool, "todo"),
            (&AskTool as &dyn Tool, "ask"),
            (&MessageTool as &dyn Tool, "message"),
        ] {
            assert_eq!(tool.name(), name);
            assert!(!tool.description().is_empty());
            assert_eq!(tool.schema()["type"], "object");
        }
    }

    #[test]
    fn validate_skill_content_accepts_and_rejects() {
        assert!(validate_skill_content("---\nname: a\ndescription: b\n---\nx").is_ok());
        assert!(validate_skill_content("no header").is_err());
        assert!(validate_skill_content("---\nname: a\n---\nx").is_err());
    }
}

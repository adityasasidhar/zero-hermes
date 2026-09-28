//! End-to-end tests for the `zero-hermes` binary itself.
//!
//! `src/main.rs` is CLI wiring: argument parsing, config discovery, and the
//! per-subcommand glue. None of it is reachable from library tests, so before
//! this file the whole entry point sat at zero coverage — every refactor of
//! the dispatch table, the config search path, or the gateway's safety checks
//! was unverified.
//!
//! These tests spawn the *real* binary in a throwaway HOME. That matters:
//! config discovery, `.env` loading, skills lookup and the sqlite memory path
//! are all functions of the environment, and a test that called into the
//! library would silently skip the very code that decides *which* config to
//! read.
//!
//! Hermeticity rules, applied by [`Sandbox::command`]:
//!   * `env_clear()` — the developer's `MINIMAX_API_KEY` and
//!     `ZERO_HERMES_CONFIG` must not leak in.
//!   * `HOME`/`XDG_*` point at the temp dir, so the real
//!     `~/.config/zero-hermes/zero_hermes.toml` and the real memory database
//!     are never read or written.
//!   * `current_dir` is the temp dir, so `./.env`, `./zero_hermes.toml` and
//!     `./skills` do not leak in either.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_zero-hermes");

/// A throwaway home directory plus a helper to run the binary inside it.
struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Write `contents` to `rel` inside the sandbox and return the full path.
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.path().join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(&p, contents).expect("write sandbox file");
        p
    }

    /// A fully isolated `Command` for the binary.
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(BIN);
        cmd.args(args)
            .current_dir(self.path())
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", self.path())
            .env("XDG_CONFIG_HOME", self.path().join("xdg-config"))
            .env("XDG_DATA_HOME", self.path().join("xdg-data"))
            .env("XDG_CACHE_HOME", self.path().join("xdg-cache"))
            // The web/gateway paths run a tracing subscriber by default;
            // silence it so assertions see only real program output.
            .env("RUST_LOG", "off")
            .env("NO_COLOR", "1");
        // The binary is spawned as a child process, so coverage tooling cannot
        // see it unless the instrumentation settings survive `env_clear()`.
        // Passing the profile path through is a no-op in a normal test run and
        // lets `-C instrument-coverage` attribute the CLI's own code.
        if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
            cmd.env("LLVM_PROFILE_FILE", profile);
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"))
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

/// Kill a spawned server even if an assertion panics mid-test.
struct KillOnDrop(Child);

impl KillOnDrop {
    fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

const ALLOWLIST_CONFIG: &str = "\
[agent]\nenabled_tools = [\"bash\", \"read\"]\n";

const CRON_CONFIG: &str = "\
[[cron.jobs]]\n\
name = \"morning\"\n\
schedule = \"0 9 * * *\"\n\
prompt = \"say good morning\"\n\
\n\
[[cron.jobs]]\n\
name = \"hourly-sweep\"\n\
schedule = \"0 * * * *\"\n\
prompt = \"sweep\"\n";

// ---------------------------------------------------------------------------
// Argument handling
// ---------------------------------------------------------------------------

#[test]
fn help_lists_every_subcommand() {
    let sb = Sandbox::new();
    let out = sb.run(&["--help"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for sub in [
        "gateway",
        "run",
        "chat",
        "cron",
        "tools",
        "init-config",
        "show-config",
        "web",
    ] {
        assert!(
            text.contains(sub),
            "`--help` should mention `{sub}`:\n{text}"
        );
    }
    // `repl` is a visible alias for `chat` and is part of the documented UI.
    assert!(
        text.contains("repl"),
        "`--help` should advertise the chat alias"
    );
}

#[test]
fn version_flag_reports_the_crate_version() {
    let sb = Sandbox::new();
    let out = sb.run(&["--version"]);
    assert_eq!(code(&out), 0);
    let text = stdout(&out);
    assert!(
        text.contains("zero-hermes"),
        "`--version` should name the binary: {text}"
    );
    assert!(
        text.contains(env!("CARGO_PKG_VERSION")),
        "`--version` should report the crate version: {text}"
    );
}

#[test]
fn a_missing_subcommand_is_a_usage_error() {
    let sb = Sandbox::new();
    let out = sb.run(&[]);
    assert_ne!(code(&out), 0, "no subcommand should not succeed");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("Usage") || text.contains("usage"),
        "a bare invocation should print usage:\n{text}"
    );
}

#[test]
fn an_unknown_subcommand_fails_loudly() {
    let sb = Sandbox::new();
    let out = sb.run(&["definitely-not-a-command"]);
    assert_ne!(code(&out), 0);
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("definitely-not-a-command"),
        "the error should echo the bad input:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// run / chat against the mock provider
// ---------------------------------------------------------------------------

#[test]
fn run_with_mock_prints_the_answer_and_exits_cleanly() {
    let sb = Sandbox::new();
    let out = sb.run(&["run", "--mock", "say hi"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("hello from mock"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn run_with_mock_and_stream_prints_the_same_answer() {
    // `--stream` takes a completely different code path through
    // `agent::run_stream`; both must produce the text (stream mode prints
    // deltas instead of a single final line, hence no trailing newline
    // guarantee).
    let sb = Sandbox::new();
    let out = sb.run(&["run", "--mock", "--stream", "say hi"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("hello from mock"),
        "stdout: {}",
        stdout(&out)
    );
}

/// The interactive REPL reads stdin until EOF. Closing stdin must make it
/// exit — a REPL that spins on a closed channel would hang the user's
/// terminal. The wait is bounded (and the child killed on timeout) so a
/// regression fails the test instead of hanging CI.
#[test]
fn chat_exits_when_stdin_is_closed() {
    use std::io::Write;

    let sb = Sandbox::new();
    let mut child = sb
        .command(&["chat", "--mock", "--no-banner"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn chat");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(b"hello\n").expect("write stdin");
    }

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            assert!(status.success(), "chat exited with {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("chat was still running 60s after stdin closed");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Config discovery and reflection
// ---------------------------------------------------------------------------

#[test]
fn show_config_emits_every_section() {
    let sb = Sandbox::new();
    let out = sb.run(&["show-config"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for section in ["[provider]", "[telegram]", "[memory]", "[agent]", "[cron]"] {
        assert!(
            text.contains(section),
            "`show-config` should print {section}:\n{text}"
        );
    }
    assert!(
        text.contains("[provider].kind") || text.contains("kind ="),
        "provider kind should be visible:\n{text}"
    );
}

#[test]
fn show_config_reflects_the_config_file_it_was_given() {
    let sb = Sandbox::new();
    let cfg = sb.write(
        "custom.toml",
        "[provider]\nmodel = \"sandbox-model-xyz\"\napi_key = \"literal-key\"\n",
    );
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "show-config"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("sandbox-model-xyz"), "stdout:\n{text}");
    // A literal (non-`${VAR}`) key must survive expansion untouched.
    assert!(text.contains("literal-key"), "stdout:\n{text}");
}

/// The documented search order is
/// `--config` > `ZERO_HERMES_CONFIG` > `~/.config/...` > `./zero_hermes.toml`.
/// Each layer is exercised here because getting this wrong means the binary
/// silently reads a config the user did not intend.
#[test]
fn config_discovery_prefers_the_more_specific_source() {
    let sb = Sandbox::new();

    // 1. Bare fallback: `./zero_hermes.toml` in the working directory.
    sb.write("zero_hermes.toml", "[provider]\nmodel = \"from-cwd\"\n");
    let out = sb.run(&["show-config"]);
    assert!(
        stdout(&out).contains("from-cwd"),
        "cwd fallback failed:\n{}",
        stdout(&out)
    );

    // 2. An explicit ZERO_HERMES_CONFIG beats the cwd file.
    let env_cfg = sb.write("env.toml", "[provider]\nmodel = \"from-env\"\n");
    let out = sb
        .command(&["show-config"])
        .env("ZERO_HERMES_CONFIG", &env_cfg)
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.contains("from-env"), "env var ignored:\n{text}");
    assert!(
        !text.contains("from-cwd"),
        "env var should win over cwd:\n{text}"
    );

    // 3. `--config` beats ZERO_HERMES_CONFIG.
    let flag_cfg = sb.write("flag.toml", "[provider]\nmodel = \"from-flag\"\n");
    let out = sb
        .command(&["--config", flag_cfg.to_str().unwrap(), "show-config"])
        .env("ZERO_HERMES_CONFIG", &env_cfg)
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.contains("from-flag"), "--config ignored:\n{text}");
    assert!(!text.contains("from-env"), "--config should win:\n{text}");
}

#[test]
fn environment_references_in_config_are_expanded() {
    let sb = Sandbox::new();
    let cfg = sb.write(
        "secrets.toml",
        "[provider]\napi_key = \"${ZH_TEST_SANDBOX_KEY}\"\nbase_url = \"https://example.test/${ZH_TEST_SANDBOX_PATH}\"\n",
    );
    let out = sb
        .command(&["--config", cfg.to_str().unwrap(), "show-config"])
        .env("ZH_TEST_SANDBOX_KEY", "sk-from-environment")
        .env("ZH_TEST_SANDBOX_PATH", "anthropic")
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(text.contains("sk-from-environment"), "stdout:\n{text}");
    assert!(
        text.contains("https://example.test/anthropic"),
        "stdout:\n{text}"
    );
    assert!(
        !text.contains("${ZH_TEST_SANDBOX_KEY}"),
        "placeholder should be substituted:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// tools
// ---------------------------------------------------------------------------

#[test]
fn tools_lists_the_default_registry() {
    let sb = Sandbox::new();
    let out = sb.run(&["tools", "--mock"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    for tool in [
        "bash", "read", "write", "fetch", "memory", "skill", "subagent",
    ] {
        assert!(text.contains(tool), "`tools` should list `{tool}`:\n{text}");
    }
}

/// `[agent].enabled_tools` is an allow-list. `tools` is documented to reflect
/// the *loaded* config, so this is the one place a user can check that their
/// allow-list took effect without starting a turn.
#[test]
fn tools_honours_the_enabled_tools_allowlist() {
    let sb = Sandbox::new();
    let cfg = sb.write("allow.toml", ALLOWLIST_CONFIG);
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "tools", "--mock"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("- bash"),
        "allow-listed tool missing:\n{text}"
    );
    assert!(
        text.contains("- read"),
        "allow-listed tool missing:\n{text}"
    );
    assert!(
        !text.contains("- write"),
        "a tool outside the allow-list must not be registered:\n{text}"
    );
    assert!(
        text.contains("- subagent"),
        "subagent is documented to survive the allow-list:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// cron
// ---------------------------------------------------------------------------

#[test]
fn cron_list_prints_each_job_and_its_next_firing() {
    let sb = Sandbox::new();
    let cfg = sb.write("cron.toml", CRON_CONFIG);
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "cron", "list"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("morning"), "stdout:\n{text}");
    assert!(text.contains("0 9 * * *"), "stdout:\n{text}");
    assert!(text.contains("hourly-sweep"), "stdout:\n{text}");
    assert!(
        text.contains("->"),
        "each job should show its next firing time:\n{text}"
    );
}

#[test]
fn cron_list_on_defaults_is_empty_but_succeeds() {
    let sb = Sandbox::new();
    let out = sb.run(&["cron", "list"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("Configured cron jobs"));
}

#[test]
fn cron_check_reports_a_next_firing_time() {
    let sb = Sandbox::new();
    let out = sb.run(&["cron", "check", "*/5 * * * *"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("*/5 * * * *"), "stdout:\n{text}");
    assert!(text.contains("next:"), "stdout:\n{text}");
}

#[test]
fn cron_check_rejects_a_garbage_expression() {
    let sb = Sandbox::new();
    let out = sb.run(&["cron", "check", "not-a-cron-expression"]);
    assert_ne!(code(&out), 0, "garbage cron should be an error");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        !text.contains("next:"),
        "a bad expression must not print a firing time:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// init-config
// ---------------------------------------------------------------------------

#[test]
fn init_config_writes_a_sample_and_refuses_to_clobber_it() {
    let sb = Sandbox::new();

    let first = sb.run(&["init-config"]);
    assert_eq!(code(&first), 0, "stderr: {}", stderr(&first));
    let written = sb.path().join("xdg-config/zero-hermes/zero_hermes.toml");
    assert!(written.is_file(), "expected {} to exist", written.display());
    let body = std::fs::read_to_string(&written).unwrap();
    assert!(
        body.contains("[provider]") && body.contains("[telegram]"),
        "sample config looks wrong:\n{body}"
    );

    // A second run must refuse rather than silently overwrite user edits.
    let second = sb.run(&["init-config"]);
    assert_ne!(code(&second), 0, "second init-config should fail");
    assert!(
        format!("{}{}", stdout(&second), stderr(&second)).contains("--force"),
        "the refusal should point at --force"
    );

    // ...and the written file must be untouched.
    assert_eq!(std::fs::read_to_string(&written).unwrap(), body);

    // --force overwrites.
    let third = sb.run(&["init-config", "--force"]);
    assert_eq!(code(&third), 0, "stderr: {}", stderr(&third));
}

/// `init-config` output must be loadable. A sample that the binary cannot
/// parse is a trap for every new user, and it is easy to break by hand.
#[test]
fn the_generated_sample_config_round_trips() {
    let sb = Sandbox::new();
    assert_eq!(code(&sb.run(&["init-config"])), 0);
    let written = sb.path().join("xdg-config/zero-hermes/zero_hermes.toml");
    let out = sb.run(&["--config", written.to_str().unwrap(), "show-config"]);
    assert_eq!(
        code(&out),
        0,
        "the generated config must load; stderr: {}",
        stderr(&out)
    );
}

// ---------------------------------------------------------------------------
// gateway safety gates
// ---------------------------------------------------------------------------

/// The gateway is remote code execution by design (the agent has a shell
/// tool). Both of these refusals are load-bearing security checks, so they
/// are pinned by tests rather than left to a code comment.
#[test]
fn gateway_refuses_to_start_without_a_token() {
    let sb = Sandbox::new();
    let out = sb.run(&["gateway"]);
    assert_ne!(code(&out), 0, "gateway without a token must not start");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("telegram.token"),
        "the error should name the missing setting:\n{text}"
    );
    assert!(
        text.contains("--mock"),
        "the error should mention the --mock escape hatch:\n{text}"
    );
}

#[test]
fn gateway_refuses_an_open_bot_without_explicit_opt_in() {
    let sb = Sandbox::new();
    let cfg = sb.write(
        "open.toml",
        "[telegram]\ntoken = \"123456:fake-token\"\nallowed_chats = []\n",
    );
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "gateway"]);
    assert_ne!(code(&out), 0, "an open bot must not start by default");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("allowed_chats"),
        "the error should name `allowed_chats`:\n{text}"
    );
    assert!(
        text.contains("allow_all_chats"),
        "the error should name the opt-in flag:\n{text}"
    );
}

/// With the token and allow-list satisfied, the gateway must reach the rest
/// of its startup and fail on the *next* genuine misconfiguration. This
/// proves the safety gates are not simply rejecting everything.
#[test]
fn gateway_reports_duplicate_cron_job_names() {
    let sb = Sandbox::new();
    let cfg = sb.write(
        "dup.toml",
        "[telegram]\n\
         token = \"123456:fake-token\"\n\
         allowed_chats = [42]\n\
         \n\
         [[cron.jobs]]\n\
         name = \"dup\"\n\
         schedule = \"0 * * * *\"\n\
         prompt = \"one\"\n\
         \n\
         [[cron.jobs]]\n\
         name = \"dup\"\n\
         schedule = \"30 * * * *\"\n\
         prompt = \"two\"\n",
    );
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "gateway"]);
    assert_ne!(code(&out), 0, "duplicate job names must be rejected");
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("duplicate cron job name"),
        "expected a duplicate-name error, got:\n{text}"
    );
}

#[test]
fn gateway_rejects_an_invalid_cron_schedule() {
    let sb = Sandbox::new();
    let cfg = sb.write(
        "badcron.toml",
        "[telegram]\n\
         token = \"123456:fake-token\"\n\
         allowed_chats = [42]\n\
         \n\
         [[cron.jobs]]\n\
         name = \"bad\"\n\
         schedule = \"every full moon\"\n\
         prompt = \"nope\"\n",
    );
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "gateway"]);
    assert_ne!(code(&out), 0, "a bad schedule must not start the gateway");
}

// ---------------------------------------------------------------------------
// web UI, over a real socket
// ---------------------------------------------------------------------------

/// Pull the 32-hex-char token the page injects into its hidden form field.
fn extract_csrf(html: &str) -> Option<String> {
    let mut rest = html;
    while let Some(i) = rest.find("value=\"") {
        let after = &rest[i + "value=\"".len()..];
        if let Some(end) = after.find('"') {
            let candidate = &after[..end];
            if candidate.len() == 32 && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(candidate.to_string());
            }
        }
        rest = &rest[i + "value=\"".len()..];
    }
    None
}

/// Exercises `serve()` for real: bind a socket, serve the page, and drive the
/// CSRF-protected `/send` from outside the process. The existing `web_test`
/// covers the router in-process; this covers the wiring that feeds it —
/// config → provider → registry → `AppState` → listening socket.
#[tokio::test]
async fn web_ui_serves_health_and_enforces_csrf_over_a_real_socket() {
    let sb = Sandbox::new();
    let port = free_port();
    let bind = format!("127.0.0.1:{port}");

    let out_log = std::fs::File::create(sb.path().join("web.out")).unwrap();
    let err_log = std::fs::File::create(sb.path().join("web.err")).unwrap();
    let child = sb
        .command(&["web", "--mock", "--bind", &bind])
        .stdout(Stdio::from(out_log))
        .stderr(Stdio::from(err_log))
        .spawn()
        .expect("spawn web server");
    let mut guard = KillOnDrop(child);

    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("http client");

    // Poll for readiness rather than sleeping a fixed amount.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut healthy = false;
    while Instant::now() < deadline {
        if let Ok(resp) = client.get(format!("{base}/health")).send().await {
            if resp.status().is_success() {
                healthy = resp.text().await.unwrap_or_default().contains("ok");
                if healthy {
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !healthy {
        let logs = format!(
            "stdout:\n{}\nstderr:\n{}",
            std::fs::read_to_string(sb.path().join("web.out")).unwrap_or_default(),
            std::fs::read_to_string(sb.path().join("web.err")).unwrap_or_default()
        );
        panic!("web UI never became healthy at {base}\n{logs}");
    }

    // The page must carry a live token, not the un-substituted placeholder.
    let html = client
        .get(format!("{base}/"))
        .send()
        .await
        .expect("GET /")
        .text()
        .await
        .expect("index body");
    assert!(html.contains("<textarea"), "index should serve the chat UI");
    assert!(
        !html.contains("{{CSRF_TOKEN}}"),
        "placeholder must be substituted"
    );
    let token = extract_csrf(&html).expect("a 32-hex csrf token in the page");

    // A cross-site form POST (simple request, no preflight) must be refused.
    let resp = client
        .post(format!("{base}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("message=rm%20-rf%20%2F")
        .send()
        .await
        .expect("POST /send without token");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "un-tokened /send must be refused"
    );

    // Guessing a token must not work either.
    let resp = client
        .post(format!("{base}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("csrf=00000000000000000000000000000000&message=hello")
        .send()
        .await
        .expect("POST /send with a guessed token");
    assert_eq!(resp.status().as_u16(), 403, "a wrong token must be refused");

    // The token the page handed out is accepted.
    let resp = client
        .post(format!("{base}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={token}&message=hello"))
        .send()
        .await
        .expect("POST /send with the real token");
    assert!(
        !resp.status().is_client_error(),
        "a correctly tokened /send should be accepted, got {}",
        resp.status()
    );

    // /events must be a live SSE stream (the UI's entire update path).
    let resp = client
        .get(format!("{base}/events"))
        .send()
        .await
        .expect("GET /events");
    assert!(resp.status().is_success(), "/events should return 200");
    let ctype = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        ctype.contains("text/event-stream"),
        "/events content-type should be text/event-stream, got {ctype:?}"
    );

    guard.kill();
}

//! Edge-case and safety tests for the builtin tools.
//!
//! The inline unit tests in `src/tools/builtin.rs` cover each tool's happy
//! path. This file covers what those miss: the failure branches the model
//! actually hits (`execute_code` with neither input, a command that exits
//! non-zero, a page with no readable text), the caps and clamps, and — most
//! importantly — that a model-supplied *name* cannot be used to write outside
//! the skills root.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use zero_hermes::agent::tool::{Tool, ToolContext, ToolOutput};
use zero_hermes::memory::Memory;
use zero_hermes::tools::builtin::{
    BashTool, ExecuteCodeTool, ReadFileTool, SearchFilesTool, SkillTool, SkillViewTool, TodoTool,
    WebExtractTool, WriteFileTool,
};

/// Run a tool that is expected to succeed, panicking on a hard error.
async fn call(tool: &dyn Tool, input: Value, ctx: &ToolContext) -> ToolOutput {
    tool.execute(input, ctx)
        .await
        .unwrap_or_else(|e| panic!("{} returned a hard error: {e:#}", tool.name()))
}

/// Run a tool that must refuse. Returns the message it refused with. Accepts
/// either style — `Err` or `Ok(ToolOutput::err(..))` — because the builtins
/// use both and the point of the assertion is the refusal, not the shape.
async fn expect_rejection(tool: &dyn Tool, input: Value, ctx: &ToolContext) -> String {
    match tool.execute(input.clone(), ctx).await {
        Err(e) => format!("{e:#}"),
        Ok(out) if out.is_error => out.content,
        Ok(out) => panic!(
            "{} should have refused {:?}, but returned success: {}",
            tool.name(),
            input,
            out.content
        ),
    }
}

fn ctx_in(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: Some(dir.to_path_buf()),
        ..Default::default()
    }
}

/// A one-shot HTTP server. Returns the URL to request.
async fn serve_once(
    status: &'static str,
    content_type: &'static str,
    body: &'static str,
) -> String {
    let (port, _url) = serve_many(1, status, content_type, body).await;
    format!("http://127.0.0.1:{port}/")
}

/// A server that answers `count` requests, returning the port it bound.
///
/// Needed by the SSRF tests, which hit one port under several different
/// spellings of `127.0.0.1`.
async fn serve_many(
    count: usize,
    status: &'static str,
    content_type: &'static str,
    body: &'static str,
) -> (u16, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        for _ in 0..count {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut scratch = [0u8; 2048];
                let _ = sock.read(&mut scratch).await;
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });
    let port = addr.port();
    (port, format!("http://127.0.0.1:{port}/"))
}

// ---------------------------------------------------------------------------
// bash / execute_command
// ---------------------------------------------------------------------------

/// A failing command is reported as *output with an exit-code note*, not as a
/// tool error. The design intent is that the model sees stdout/stderr and the
/// exit status together; the contract is pinned here because a future change
/// to `is_error` would silently change how the model reacts to failures.
#[tokio::test]
async fn bash_merges_stderr_and_appends_a_nonzero_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &BashTool,
        json!({"command": "echo to-stdout; echo to-stderr >&2; exit 3"}),
        &ctx_in(tmp.path()),
    )
    .await;

    assert!(
        !out.is_error,
        "output is a payload, not an error: {}",
        out.content
    );
    assert!(out.content.contains("to-stdout"), "{}", out.content);
    assert!(out.content.contains("to-stderr"), "{}", out.content);
    assert!(
        out.content.contains("[exit code: 3]"),
        "the exit status must be visible to the model: {}",
        out.content
    );
}

#[tokio::test]
async fn bash_reports_a_successful_command_without_an_exit_note() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &BashTool,
        json!({"command": "echo fine"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(out.content.contains("fine"));
    assert!(
        !out.content.contains("[exit code:"),
        "exit 0 should not be annotated: {}",
        out.content
    );
}

/// A command that hangs must be killed, not waited on forever. This is the
/// difference between a wedged tool call and a stuck gateway.
///
/// The bound is not a perf assertion: the command sleeps 30s and the timeout
/// is 1s, so anything near 30s means the timeout did not actually stop the
/// work.
#[tokio::test]
async fn bash_timeout_returns_promptly_and_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let out = call(
        &BashTool,
        json!({"command": "sleep 30", "timeout": 1}),
        &ctx_in(tmp.path()),
    )
    .await;
    let elapsed = started.elapsed();

    assert!(
        out.content.contains("[timeout after"),
        "a timed-out command must say so: {}",
        out.content
    );
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "a 1s timeout must return in about a second, took {elapsed:?}"
    );
}

/// `timeout` must kill the command's *descendants*, not just the shell.
///
/// `/bin/sh -c` forks, so a SIGKILL aimed at the shell alone leaves the real
/// work running as an orphan — free to burn CPU forever, and holding the
/// pipes open. The command below backgrounds a writer that would leave
/// evidence after the agent has already moved on.
#[tokio::test]
async fn bash_timeout_kills_the_whole_process_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &BashTool,
        json!({
            "command": "(sleep 20; touch escaped-orphan.marker) & sleep 20",
            "timeout": 1
        }),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(out.content.contains("[timeout after"), "{}", out.content);

    // Give the orphan every chance to prove it survived.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert!(
        !tmp.path().join("escaped-orphan.marker").exists(),
        "a timed-out command left a descendant running"
    );
}

#[tokio::test]
async fn bash_runs_in_the_context_working_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("only-here.txt"), "x").unwrap();
    let out = call(&BashTool, json!({"command": "ls"}), &ctx_in(tmp.path())).await;
    assert!(
        out.content.contains("only-here.txt"),
        "cwd was not honoured: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// execute_code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_code_runs_python() {
    let tmp = tempfile::tempdir().unwrap();
    for language in ["python", "python3", "Python"] {
        let out = call(
            &ExecuteCodeTool,
            json!({"code": "print(6 * 7)", "language": language}),
            &ctx_in(tmp.path()),
        )
        .await;
        assert!(!out.is_error, "{} failed: {}", language, out.content);
        assert!(
            out.content.contains("42"),
            "`{language}` should run through python3: {}",
            out.content
        );
    }
}

/// Without `language`, a snippet is shell — that is the documented default and
/// it is what makes `execute_code` a superset of `bash`/`execute_command`.
#[tokio::test]
async fn execute_code_falls_back_to_the_shell() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &ExecuteCodeTool,
        json!({"code": "echo from-the-shell"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(out.content.contains("from-the-shell"), "{}", out.content);
}

#[tokio::test]
async fn execute_code_honours_the_working_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("marker.txt"), "x").unwrap();
    // Both the python and the shell path must respect `ctx.cwd`.
    for input in [
        json!({"code": "import os; print(sorted(os.listdir('.')))", "language": "python"}),
        json!({"code": "ls"}),
    ] {
        let out = call(&ExecuteCodeTool, input.clone(), &ctx_in(tmp.path())).await;
        assert!(
            out.content.contains("marker.txt"),
            "cwd not honoured for {input}: {}",
            out.content
        );
    }
}

#[tokio::test]
async fn execute_code_accepts_command_as_an_alternative_to_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &ExecuteCodeTool,
        json!({"command": "echo via-command"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(out.content.contains("via-command"), "{}", out.content);
}

/// Calling it with nothing to run is a model mistake, not a crash.
#[tokio::test]
async fn execute_code_requires_either_command_or_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(&ExecuteCodeTool, json!({}), &ctx_in(tmp.path())).await;
    assert!(out.is_error, "no input should be an error: {}", out.content);
    assert!(
        out.content.contains("requires"),
        "the error should explain what is missing: {}",
        out.content
    );
}

#[tokio::test]
async fn execute_code_reports_a_failing_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let out = call(
        &ExecuteCodeTool,
        json!({"command": "exit 7"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(
        out.content.contains("[exit code: 7]"),
        "the exit status must be visible: {}",
        out.content
    );
}

// ---------------------------------------------------------------------------
// search_files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_files_returns_path_line_hits() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("nested")).unwrap();
    std::fs::write(
        tmp.path().join("nested/first.txt"),
        "nothing\nhere be the needle\n",
    )
    .unwrap();
    std::fs::write(tmp.path().join("second.txt"), "needle again\n").unwrap();

    let out = call(
        &SearchFilesTool,
        json!({"pattern": "needle"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(!out.is_error, "{}", out.content);
    assert!(
        out.content.contains("nested/first.txt:2"),
        "hits should be `path:line`: {}",
        out.content
    );
    assert!(
        out.content.contains("second.txt:1"),
        "the search should recurse: {}",
        out.content
    );
}

#[tokio::test]
async fn search_files_says_so_when_nothing_matches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "contents\n").unwrap();
    let out = call(
        &SearchFilesTool,
        json!({"pattern": "definitely-not-present-xyzzy"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(!out.is_error);
    assert!(
        out.content.contains("no matches"),
        "an empty result should be explicit: {}",
        out.content
    );
}

#[tokio::test]
async fn search_files_rejects_an_empty_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let out = expect_rejection(
        &SearchFilesTool,
        json!({"pattern": ""}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(out.contains("non-empty"), "the error should say why: {out}");
}

/// `max_results` is clamped rather than trusted: 0 must still return a hit
/// rather than an empty list, and a huge value must not be able to ask for
/// unlimited output.
#[tokio::test]
async fn search_files_clamps_max_results() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "needle\nneedle\nneedle\n").unwrap();

    let zero = call(
        &SearchFilesTool,
        json!({"pattern": "needle", "max_results": 0}),
        &ctx_in(tmp.path()),
    )
    .await;
    let hits = zero.content.lines().filter(|l| l.contains("a.txt")).count();
    assert_eq!(hits, 1, "0 should clamp up to 1, got: {}", zero.content);

    let one = call(
        &SearchFilesTool,
        json!({"pattern": "needle", "max_results": 1}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert_eq!(
        one.content.lines().filter(|l| l.contains("a.txt")).count(),
        1
    );
}

// ---------------------------------------------------------------------------
// web_extract / fetch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn web_extract_strips_html_tags_and_scripts() {
    let url = serve_once(
        "200 OK",
        "text/html",
        "<html><head><style>p{color:red}</style></head><body>\
         <script>alert('nope')</script><h1>Title</h1><p>Body &amp; more</p></body></html>",
    )
    .await;

    let out = call(
        &WebExtractTool::with_private_allowed(),
        json!({"url": url}),
        &ToolContext::default(),
    )
    .await;
    assert!(!out.is_error, "{}", out.content);
    assert!(out.content.contains("Title"), "{}", out.content);
    assert!(out.content.contains("Body"), "{}", out.content);
    // Entities are decoded...
    assert!(out.content.contains("Body & more"), "{}", out.content);
    // ...and markup, scripts and styles are gone.
    assert!(!out.content.contains("<p>"), "{}", out.content);
    assert!(!out.content.contains("alert"), "{}", out.content);
    assert!(!out.content.contains("color:red"), "{}", out.content);
}

#[tokio::test]
async fn web_extract_reports_a_page_with_no_readable_text() {
    let url = serve_once("200 OK", "text/html", "<html><body></body></html>").await;
    let out = expect_rejection(
        &WebExtractTool::with_private_allowed(),
        json!({"url": url}),
        &ToolContext::default(),
    )
    .await;
    assert!(
        out.contains("no readable text"),
        "the model should be told why it got nothing: {out}"
    );
}

#[tokio::test]
async fn web_extract_requires_a_url() {
    let out = expect_rejection(
        &WebExtractTool::default(),
        json!({"url": "   "}),
        &ToolContext::default(),
    )
    .await;
    assert!(out.contains("url"), "error should name the field: {out}");
}

/// `fetch` surfaces the HTTP status as text instead of erroring, so the model
/// can reason about a 404 or a 500. Pinned because the alternative (an error
/// with no body) loses the diagnostic.
#[tokio::test]
async fn fetch_reports_the_status_instead_of_hiding_it() {
    let url = serve_once("404 Not Found", "text/plain", "no such page").await;
    let out = call(
        &zero_hermes::tools::builtin::FetchTool::with_private_allowed(),
        json!({"url": url}),
        &ToolContext::default(),
    )
    .await;
    assert!(!out.is_error, "{}", out.content);
    assert!(out.content.contains("404"), "{}", out.content);
    assert!(out.content.contains("no such page"), "{}", out.content);
}

/// SSRF guard. `fetch` takes a model-supplied URL, so a request for loopback
/// or a cloud metadata address must never leave the process.
#[tokio::test]
async fn fetch_refuses_loopback_and_private_hosts() {
    let tool = zero_hermes::tools::builtin::FetchTool::default();
    for url in [
        "http://127.0.0.1:1/nothing",
        "http://localhost:8080/",
        "http://[::1]:8080/",
        "http://10.0.0.1/",
        "http://192.168.1.1/",
        // The cloud metadata endpoint: the address an SSRF is usually after.
        "http://169.254.169.254/latest/meta-data/",
        // Non-http schemes must not be smuggled through either.
        "file:///etc/passwd",
    ] {
        let out = expect_rejection(&tool, json!({"url": url}), &ToolContext::default()).await;
        assert!(
            out.contains("refusing to fetch") || out.contains("blocked"),
            "{url} should be blocked, got: {out}"
        );
    }
}

/// The guard is only worth anything if every tool that performs the fetch has
/// it. `web_extract` issues the same GET as `fetch` against the same
/// model-supplied URL, so a guard on just one of them is bypassable by
/// picking the other name.
#[tokio::test]
async fn web_extract_is_subject_to_the_same_ssrf_guard_as_fetch() {
    let tool = WebExtractTool::default();
    for url in [
        "http://127.0.0.1:1/nothing",
        "http://localhost:8080/",
        "http://169.254.169.254/latest/meta-data/",
        "http://10.0.0.1/",
        "http://2130706433/",
        "http://[::ffff:127.0.0.1]/",
        "file:///etc/passwd",
    ] {
        let out = expect_rejection(&tool, json!({"url": url}), &ToolContext::default()).await;
        assert!(
            out.contains("refusing to fetch") || out.contains("blocked"),
            "web_extract bypasses the `fetch` SSRF guard for {url}: {out}"
        );
    }

    // ...and the test seam really does lift it, so the guard above is not
    // simply rejecting everything.
    let url = serve_once("200 OK", "text/plain", "reachable").await;
    let out = call(
        &WebExtractTool::with_private_allowed(),
        json!({"url": url}),
        &ToolContext::default(),
    )
    .await;
    assert!(out.content.contains("reachable"), "{}", out.content);
}

/// SSRF filters are normally defeated by *spelling*, not by finding a new
/// address. Every template below was confirmed on this machine to open a
/// connection to a server bound only on `127.0.0.1`, while looking nothing
/// like a dotted quad — which is why a `starts_with("127.")`-style check is
/// not a control at all.
#[tokio::test]
async fn fetch_blocks_host_encoding_evasions_of_loopback() {
    const EVASIONS: [&str; 6] = [
        // A bare 32-bit address, decimal and hex.
        "http://2130706433:{p}/",
        "http://0x7f000001:{p}/",
        // Octal first octet.
        "http://0177.0.0.1:{p}/",
        // IPv4-mapped IPv6: dotted, hex-group, and fully expanded.
        "http://[::ffff:127.0.0.1]:{p}/",
        "http://[::ffff:7f00:1]:{p}/",
        "http://[0:0:0:0:0:ffff:127.0.0.1]:{p}/",
    ];

    let (port, _) = serve_many(EVASIONS.len(), "200 OK", "text/plain", "LOCAL-CANARY").await;
    let port = port.to_string();

    // Positive control: the test seam reaches the server through every one of
    // these spellings, so the addresses really are loopback and the negative
    // assertions below are not vacuous.
    let permissive = zero_hermes::tools::builtin::FetchTool::with_private_allowed();
    for template in EVASIONS {
        let url = template.replace("{p}", &port);
        let out = call(&permissive, json!({"url": url}), &ToolContext::default()).await;
        assert!(
            out.content.contains("LOCAL-CANARY"),
            "{url} should reach the local canary server, else this test proves nothing: {}",
            out.content
        );
    }

    // The real tool refuses all of them.
    let guarding = zero_hermes::tools::builtin::FetchTool::default();
    for template in EVASIONS {
        let url = template.replace("{p}", &port);
        let out = expect_rejection(&guarding, json!({"url": url}), &ToolContext::default()).await;
        assert!(
            out.contains("refusing to fetch") || out.contains("blocked"),
            "host-encoding evasion not blocked: {url} -> {out}"
        );
    }
}

// ---------------------------------------------------------------------------
// skill / skill_view — the skills root boundary
// ---------------------------------------------------------------------------

/// A skill name is model-supplied and becomes a path. Every one of these is
/// an escape attempt; `safe_skill_name` must reject the whole class, and — the
/// part that actually matters — nothing may be created outside the root.
#[tokio::test]
async fn skill_names_cannot_escape_the_skills_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("skills");
    std::fs::create_dir_all(&root).unwrap();
    let tool = SkillTool::new(root.clone());
    let view = SkillViewTool::new(root.clone());

    let valid_content = "---\nname: x\ndescription: y\n---\nbody\n";
    let attempts = [
        "../escape",
        "..",
        "../../escape",
        "/etc/passwd",
        "a/b",
        "sub/../escape",
        "",
        "with space",
        "with.dot",
        "with\0nul",
        "with\\backslash",
    ];

    for name in attempts {
        for (label, result) in [
            (
                "read",
                expect_rejection(
                    &tool,
                    json!({"action": "read", "name": name}),
                    &ToolContext::default(),
                )
                .await,
            ),
            (
                "write",
                expect_rejection(
                    &tool,
                    json!({"action": "write", "name": name, "content": valid_content}),
                    &ToolContext::default(),
                )
                .await,
            ),
            (
                "patch",
                expect_rejection(
                    &tool,
                    json!({"action": "patch", "name": name, "old_str": "body", "new_str": "x"}),
                    &ToolContext::default(),
                )
                .await,
            ),
            (
                "skill_view",
                expect_rejection(&view, json!({"name": name}), &ToolContext::default()).await,
            ),
        ] {
            assert!(
                !result.is_empty(),
                "{label} with name {name:?} was refused but gave no reason"
            );
        }
    }

    // Nothing may have been written outside the root.
    for escaped in [
        tmp.path().join("escape"),
        tmp.path().join("escape").join("SKILL.md"),
        tmp.path().join("sub"),
    ] {
        assert!(
            !escaped.exists(),
            "a rejected skill name created {}",
            escaped.display()
        );
    }
}

#[tokio::test]
async fn skill_write_then_read_round_trips_inside_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("skills");
    let tool = SkillTool::new(root.clone());

    let content = "---\nname: learned-thing\ndescription: what it does\n---\n\nDo the thing.\n";
    let written = call(
        &tool,
        json!({"action": "write", "name": "learned-thing", "content": content}),
        &ToolContext::default(),
    )
    .await;
    assert!(!written.is_error, "{}", written.content);

    // It must land under the root...
    let expected = root.join("learned-thing").join("SKILL.md");
    assert!(expected.is_file(), "expected {}", expected.display());

    // ...and be readable back through the registry-backed lookup.
    let read = call(
        &tool,
        json!({"action": "read", "name": "learned-thing"}),
        &ToolContext::default(),
    )
    .await;
    assert!(!read.is_error, "{}", read.content);
    assert!(read.content.contains("Do the thing"), "{}", read.content);

    // ...and be listed.
    let listed = call(&tool, json!({"action": "list"}), &ToolContext::default()).await;
    assert!(
        listed.content.contains("learned-thing"),
        "{}",
        listed.content
    );
}

#[tokio::test]
async fn skill_write_rejects_content_without_valid_frontmatter() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("skills");
    let tool = SkillTool::new(root.clone());

    // No frontmatter at all, and frontmatter missing `description:`.
    for bad in ["just prose\n", "---\nname: only-a-name\n---\nbody\n"] {
        let out = expect_rejection(
            &tool,
            json!({"action": "write", "name": "bad-skill", "content": bad}),
            &ToolContext::default(),
        )
        .await;
        assert!(
            out.contains("SKILL.md") || out.contains("frontmatter"),
            "the error should explain the requirement: {out}"
        );
    }
    assert!(
        !root.join("bad-skill").exists(),
        "invalid content must not create a skill directory"
    );
}

#[tokio::test]
async fn skill_view_reports_an_unknown_name() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("skills");
    std::fs::create_dir_all(&root).unwrap();
    let view = SkillViewTool::new(root);
    let out = expect_rejection(
        &view,
        json!({"name": "no-such-skill"}),
        &ToolContext::default(),
    )
    .await;
    assert!(out.contains("unknown skill"), "{out}");
}

// ---------------------------------------------------------------------------
// read_file / write_file aliases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_file_aliases_round_trip_like_their_short_forms() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = ctx_in(tmp.path());

    let written = call(
        &WriteFileTool,
        json!({"path": "alias.txt", "content": "via alias\n"}),
        &ctx,
    )
    .await;
    assert!(!written.is_error, "{}", written.content);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("alias.txt")).unwrap(),
        "via alias\n"
    );

    let read = call(&ReadFileTool, json!({"path": "alias.txt"}), &ctx).await;
    assert_eq!(read.content, "via alias\n");

    // Overwriting is allowed (write, not append).
    call(
        &WriteFileTool,
        json!({"path": "alias.txt", "content": "replaced\n"}),
        &ctx,
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("alias.txt")).unwrap(),
        "replaced\n"
    );
}

#[tokio::test]
async fn reading_a_missing_file_fails_without_hard_erroring() {
    let tmp = tempfile::tempdir().unwrap();
    let out = expect_rejection(
        &ReadFileTool,
        json!({"path": "not-here.txt"}),
        &ctx_in(tmp.path()),
    )
    .await;
    assert!(
        out.contains("not-here.txt"),
        "error should name the path: {out}"
    );
}

// ---------------------------------------------------------------------------
// todo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn todo_reports_a_corrupt_note_instead_of_panicking() {
    let mem = Arc::new(Memory::in_memory().unwrap());
    mem.write_note("todos", "{ this is not json").unwrap();
    let ctx = ToolContext {
        memory: Some(mem),
        ..Default::default()
    };

    let out = expect_rejection(&TodoTool, json!({"action": "list"}), &ctx).await;
    assert!(
        out.contains("corrupt"),
        "a corrupt todo note should be reported with a way out: {out}"
    );
    assert!(
        out.contains("clear"),
        "the error should tell the model how to recover: {out}"
    );
}

#[tokio::test]
async fn todo_without_a_memory_backend_is_reported() {
    let out = expect_rejection(
        &TodoTool,
        json!({"action": "list"}),
        &ToolContext::default(),
    )
    .await;
    assert!(
        out.to_lowercase().contains("memory"),
        "the error should mention the missing backend: {out}"
    );
}

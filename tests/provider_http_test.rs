//! Provider tests over a real HTTP socket.
//!
//! The existing provider tests only exercise `build_body` / `parse_response`
//! as pure functions. Everything in between — URLs, auth headers, status
//! handling, body-read failures, timeouts and the streaming wiring — runs in
//! `complete()`/`stream()` and was untested. Those are exactly the paths that
//! break when someone swaps a provider or a proxy changes a status code, and
//! a unit test cannot see them.
//!
//! The mock server below answers queued replies and records the raw requests,
//! so each test can assert on both sides of the wire.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use zero_hermes::agent::provider::{build_provider, AnthropicMessages, OpenAiCompat};
use zero_hermes::agent::stream::StreamEvent;
use zero_hermes::agent::tool::{Message, ToolCall};
use zero_hermes::agent::LlmProvider;
use zero_hermes::config::{ProviderConfig, ProviderKind};

// ---------------------------------------------------------------------------
// Mock HTTP server
// ---------------------------------------------------------------------------

/// What the server should do for the next accepted connection.
#[derive(Clone, Debug)]
enum Reply {
    /// A complete response with a `Content-Length`.
    Body {
        status: u16,
        content_type: &'static str,
        body: String,
    },
    /// Headers claiming more bytes than are actually sent, then close. This
    /// is what a dropped connection looks like to a client.
    Truncated { declared_len: usize, body: String },
    /// Accept and never answer — a wedged provider.
    Hang,
}

fn json_reply(status: u16, body: impl Into<String>) -> Reply {
    Reply::Body {
        status,
        content_type: "application/json",
        body: body.into(),
    }
}

fn sse_reply(body: impl Into<String>) -> Reply {
    Reply::Body {
        status: 200,
        content_type: "text/event-stream",
        body: body.into(),
    }
}

/// A request as observed on the wire.
#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

impl Recorded {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("request body was not JSON ({e}): {}", self.body))
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

struct Server {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Recorded>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    /// Start a server that answers each connection with the next queued reply.
    async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let addr = listener.local_addr().expect("mock server addr");
        let seen: Arc<Mutex<Vec<Recorded>>> = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();

        let task = tokio::spawn(async move {
            let mut queue = replies.into_iter();
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let reply = queue
                    .next()
                    .unwrap_or_else(|| json_reply(500, r#"{"error":"no queued reply"}"#));
                let seen = seen2.clone();
                // Serve each connection to completion in its own task so a
                // `Hang` reply cannot block later connections.
                tokio::spawn(async move {
                    if let Some(recorded) = read_request(&mut sock).await {
                        seen.lock().unwrap().push(recorded);
                    }
                    match reply {
                        Reply::Body {
                            status,
                            content_type,
                            body,
                        } => {
                            let head = format!(
                                "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                status_text(status),
                                body.len()
                            );
                            let _ = sock.write_all(head.as_bytes()).await;
                            let _ = sock.write_all(body.as_bytes()).await;
                            let _ = sock.flush().await;
                        }
                        Reply::Truncated { declared_len, body } => {
                            let head = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {declared_len}\r\nConnection: close\r\n\r\n"
                            );
                            let _ = sock.write_all(head.as_bytes()).await;
                            let _ = sock.write_all(body.as_bytes()).await;
                            let _ = sock.flush().await;
                            // Drop the socket with the body short of the
                            // declared length.
                        }
                        Reply::Hang => {
                            tokio::time::sleep(Duration::from_secs(120)).await;
                        }
                    }
                });
            }
        });

        Self { addr, seen, task }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn requests(&self) -> Vec<Recorded> {
        self.seen.lock().unwrap().clone()
    }

    fn last_request(&self) -> Recorded {
        self.requests()
            .pop()
            .expect("the server should have received a request")
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Read one HTTP/1.1 request (headers + `Content-Length` body).
async fn read_request(sock: &mut TcpStream) -> Option<Recorded> {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];

    let head_end = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    };

    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default().to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let want: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    while buf.len() < head_end + want {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }

    Some(Recorded {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&buf[head_end..]).to_string(),
    })
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

fn config(kind: ProviderKind, base_url: &str) -> ProviderConfig {
    ProviderConfig {
        kind,
        base_url: base_url.to_string(),
        api_key: "test-api-key".to_string(),
        model: "test-model".to_string(),
        max_tokens: 128,
        system: None,
        temperature: None,
        connect_timeout_secs: 5,
        read_timeout_secs: 5,
        // No fallbacks: these tests assert what a single configured endpoint
        // does on failure, which a fallback would mask.
        fallbacks: Vec::new(),
    }
}

fn anthropic_cfg(base_url: &str) -> ProviderConfig {
    config(ProviderKind::Anthropic, base_url)
}

fn openai_cfg(base_url: &str) -> ProviderConfig {
    config(ProviderKind::OpenaiCompat, base_url)
}

fn user(text: &str) -> Vec<Message> {
    vec![Message::user(text)]
}

const TOOL_SCHEMAS: &str =
    r#"[{"name":"bash","description":"run","input_schema":{"type":"object"}}]"#;

fn tool_schemas() -> Vec<Value> {
    serde_json::from_str(TOOL_SCHEMAS).unwrap()
}

// ---------------------------------------------------------------------------
// Anthropic /v1/messages
// ---------------------------------------------------------------------------

#[tokio::test]
async fn anthropic_complete_posts_the_documented_request() {
    let server = Server::start(vec![json_reply(
        200,
        r#"{"content":[{"type":"text","text":"hello there"}],"stop_reason":"end_turn"}"#,
    )])
    .await;

    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let completion = provider
        .complete(Some("you are a test"), &user("hi"), &tool_schemas())
        .await
        .expect("complete");

    assert_eq!(completion.text.as_deref(), Some("hello there"));
    assert!(completion.tool_calls.is_empty());
    assert!(completion.is_final());

    let req = server.last_request();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/messages", "wrong endpoint");
    assert_eq!(req.header("x-api-key"), Some("test-api-key"));
    assert_eq!(req.header("anthropic-version"), Some("2023-06-01"));
    assert!(
        req.header("content-type")
            .is_some_and(|v| v.starts_with("application/json")),
        "content-type: {:?}",
        req.header("content-type")
    );

    let body = req.json();
    assert_eq!(
        body["model"], "test-model",
        "model must come from the config"
    );
    assert_eq!(body["max_tokens"], 128);
    // The system prompt goes out as a *cacheable block array*, not a plain
    // string, so Anthropic prompt caching can reuse the prefix across turns.
    // Losing the `cache_control` block silently disables caching (and raises
    // cost) without breaking a single response parse, which is why it is
    // asserted here and not only on the in-memory Value.
    assert_eq!(body["system"][0]["type"], "text");
    assert_eq!(body["system"][0]["text"], "you are a test");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"][0]["text"], "hi");
    assert_eq!(body["tools"][0]["name"], "bash");
}

/// A trailing slash in `base_url` is a common copy-paste artifact; it must not
/// produce `//v1/messages`.
#[tokio::test]
async fn anthropic_complete_tolerates_a_trailing_slash_in_the_base_url() {
    let server = Server::start(vec![json_reply(
        200,
        r#"{"content":[{"type":"text","text":"ok"}]}"#,
    )])
    .await;

    let base = format!("{}/", server.url());
    let provider = AnthropicMessages::new(&anthropic_cfg(&base)).unwrap();
    provider.complete(None, &user("hi"), &[]).await.unwrap();
    assert_eq!(server.last_request().path, "/v1/messages");
}

#[tokio::test]
async fn anthropic_complete_parses_tool_use_into_tool_calls() {
    let server = Server::start(vec![json_reply(
        200,
        r#"{"content":[
            {"type":"text","text":"let me look"},
            {"type":"tool_use","id":"toolu_1","name":"bash","input":{"command":"ls -la"}}
        ]}"#,
    )])
    .await;

    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let completion = provider
        .complete(None, &user("list files"), &[])
        .await
        .unwrap();

    assert_eq!(completion.tool_calls.len(), 1);
    let tc: &ToolCall = &completion.tool_calls[0];
    assert_eq!(tc.id, "toolu_1");
    assert_eq!(tc.name, "bash");
    assert_eq!(tc.input["command"], "ls -la");
    // Text alongside a tool call is preserved, but this is not a final answer.
    assert_eq!(completion.text.as_deref(), Some("let me look"));
    assert!(!completion.is_final());
}

/// A 429 or 500 is the single most common provider failure. The error must
/// name the status and include some of the body, otherwise the operator has
/// nothing to act on.
#[tokio::test]
async fn anthropic_complete_reports_a_non_success_status_with_a_body_excerpt() {
    let server = Server::start(vec![json_reply(
        429,
        r#"{"error":{"type":"rate_limit_error","message":"slow down please"}}"#,
    )])
    .await;

    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let err = provider
        .complete(None, &user("hi"), &[])
        .await
        .expect_err("a 429 must not look like success");
    let msg = format!("{err:#}");
    assert!(msg.contains("429"), "error should name the status: {msg}");
    assert!(
        msg.contains("slow down please"),
        "error should quote the body: {msg}"
    );
}

/// A gateway that returns an HTML error page (proxy, captive portal) must be
/// reported as a parse failure, not a panic.
#[tokio::test]
async fn anthropic_complete_reports_a_non_json_body() {
    let server = Server::start(vec![Reply::Body {
        status: 200,
        content_type: "text/html",
        body: "<html><body>502 Bad Gateway</body></html>".to_string(),
    }])
    .await;

    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let err = provider
        .complete(None, &user("hi"), &[])
        .await
        .expect_err("HTML is not a completion");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("not JSON") || msg.contains("expected"),
        "error should explain the parse failure: {msg}"
    );
}

#[tokio::test]
async fn a_connection_error_is_reported_not_panicked() {
    // Nothing is listening on this port.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let provider =
        AnthropicMessages::new(&anthropic_cfg(&format!("http://127.0.0.1:{port}"))).unwrap();
    let err = provider
        .complete(None, &user("hi"), &[])
        .await
        .expect_err("a refused connection must be an error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("LLM request failed"),
        "error should be the provider's own wording: {msg}"
    );
}

/// The read timeout is what stops a wedged provider from taking the whole
/// gateway down with it, so it needs a test that actually waits on a socket
/// that accepts and then says nothing.
#[tokio::test]
async fn a_provider_that_stops_talking_times_out_instead_of_hanging() {
    let server = Server::start(vec![Reply::Hang]).await;
    let mut cfg = anthropic_cfg(&server.url());
    cfg.read_timeout_secs = 1;
    cfg.connect_timeout_secs = 1;

    let provider = AnthropicMessages::new(&cfg).unwrap();
    let started = Instant::now();
    let err = tokio::time::timeout(
        Duration::from_secs(20),
        provider.complete(None, &user("hi"), &[]),
    )
    .await
    .expect("the read timeout should have fired; the call hung instead")
    .expect_err("a silent provider must not be treated as success");

    assert!(
        started.elapsed() < Duration::from_secs(15),
        "should fail promptly on the configured 1s read timeout, took {:?}",
        started.elapsed()
    );
    let msg = format!("{err:#}");
    assert!(
        msg.contains("LLM request failed") || msg.contains("timed out"),
        "error should describe the timeout: {msg}"
    );
}

// ---------------------------------------------------------------------------
// OpenAI-compatible /v1/chat/completions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn openai_complete_uses_bearer_auth_and_parses_content() {
    let server = Server::start(vec![json_reply(
        200,
        r#"{"choices":[{"message":{"role":"assistant","content":"hi from openai"}}]}"#,
    )])
    .await;

    let provider = OpenAiCompat::new(&openai_cfg(&server.url())).unwrap();
    let completion = provider
        .complete(Some("sys"), &user("hello"), &tool_schemas())
        .await
        .expect("complete");
    assert_eq!(completion.text.as_deref(), Some("hi from openai"));

    let req = server.last_request();
    assert_eq!(req.path, "/v1/chat/completions");
    assert_eq!(
        req.header("authorization"),
        Some("Bearer test-api-key"),
        "OpenAI-compatible endpoints authenticate with a bearer token"
    );
    // The Anthropic-specific header must not leak onto this wire format.
    assert_eq!(req.header("x-api-key"), None);

    let body = req.json();
    assert_eq!(body["model"], "test-model");
    assert_eq!(
        body["system"],
        Value::Null,
        "system is sent as a message, not a field"
    );
    // System prompt becomes the first message for this wire format.
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "sys");
    assert_eq!(body["messages"][1]["content"], "hello");
}

#[tokio::test]
async fn openai_complete_sends_temperature_only_when_configured() {
    let server = Server::start(vec![
        json_reply(200, r#"{"choices":[{"message":{"content":"a"}}]}"#),
        json_reply(200, r#"{"choices":[{"message":{"content":"b"}}]}"#),
    ])
    .await;

    let mut cfg = openai_cfg(&server.url());
    let provider = OpenAiCompat::new(&cfg).unwrap();
    provider.complete(None, &user("hi"), &[]).await.unwrap();
    let without = server.last_request().json();
    assert!(
        without.get("temperature").is_none(),
        "an unset temperature must not be sent: {without}"
    );

    cfg.temperature = Some(0.25);
    let provider = OpenAiCompat::new(&cfg).unwrap();
    provider.complete(None, &user("hi"), &[]).await.unwrap();
    let with = server.last_request().json();
    assert_eq!(with["temperature"], 0.25);
}

#[tokio::test]
async fn openai_complete_parses_tool_calls_with_string_arguments() {
    let server = Server::start(vec![json_reply(
        200,
        r#"{"choices":[{"message":{"role":"assistant","content":null,
            "tool_calls":[{"id":"call_1","type":"function",
              "function":{"name":"bash","arguments":"{\"command\":\"pwd\"}"}}]}}]}"#,
    )])
    .await;

    let provider = OpenAiCompat::new(&openai_cfg(&server.url())).unwrap();
    let completion = provider
        .complete(None, &user("where am i"), &[])
        .await
        .unwrap();
    assert_eq!(completion.tool_calls.len(), 1);
    assert_eq!(completion.tool_calls[0].id, "call_1");
    assert_eq!(completion.tool_calls[0].name, "bash");
    assert_eq!(completion.tool_calls[0].input["command"], "pwd");
}

#[tokio::test]
async fn openai_complete_reports_a_non_success_status() {
    let server = Server::start(vec![json_reply(
        401,
        r#"{"error":{"message":"invalid api key"}}"#,
    )])
    .await;
    let provider = OpenAiCompat::new(&openai_cfg(&server.url())).unwrap();
    let err = provider
        .complete(None, &user("hi"), &[])
        .await
        .expect_err("401 must be an error");
    let msg = format!("{err:#}");
    assert!(msg.contains("401"), "error: {msg}");
    assert!(msg.contains("invalid api key"), "error: {msg}");
}

/// The whole documented point of `kind`: the same config shape must select a
/// different wire format. This checks the factory by observing which endpoint
/// the traffic actually reached.
#[tokio::test]
async fn build_provider_selects_the_wire_format_from_the_kind() {
    let server = Server::start(vec![
        json_reply(200, r#"{"content":[{"type":"text","text":"anthropic"}]}"#),
        json_reply(200, r#"{"choices":[{"message":{"content":"openai"}}]}"#),
    ])
    .await;

    let anthropic = build_provider(&anthropic_cfg(&server.url())).unwrap();
    let out = anthropic.complete(None, &user("hi"), &[]).await.unwrap();
    assert_eq!(out.text.as_deref(), Some("anthropic"));
    assert_eq!(server.last_request().path, "/v1/messages");

    let openai = build_provider(&openai_cfg(&server.url())).unwrap();
    let out = openai.complete(None, &user("hi"), &[]).await.unwrap();
    assert_eq!(out.text.as_deref(), Some("openai"));
    assert_eq!(server.last_request().path, "/v1/chat/completions");
}

// ---------------------------------------------------------------------------
// Streaming, end to end
// ---------------------------------------------------------------------------

const ANTHROPIC_SSE: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\"}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"streamed \"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"answer\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);

const OPENAI_SSE: &str = concat!(
    "data: {\"choices\":[{\"delta\":{\"content\":\"streamed \"}}]}\n\n",
    "data: {\"choices\":[{\"delta\":{\"content\":\"answer\"}}]}\n\n",
    "data: [DONE]\n\n",
);

/// Collect a whole event stream, failing instead of hanging if it never ends.
async fn collect<S>(mut s: S) -> Vec<StreamEvent>
where
    S: futures::Stream<Item = zero_hermes::error::Result<StreamEvent>> + Unpin,
{
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(20), s.next()).await {
            Ok(Some(Ok(ev))) => out.push(ev),
            Ok(Some(Err(e))) => panic!("stream error: {e:#}"),
            Ok(None) => return out,
            Err(_) => panic!("stream never terminated; got {out:?}"),
        }
    }
}

#[tokio::test]
async fn anthropic_stream_delivers_text_deltas_and_a_final_completion() {
    let server = Server::start(vec![sse_reply(ANTHROPIC_SSE)]).await;
    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let messages = user("hi");
    let tools: Vec<Value> = Vec::new();

    let events = collect(provider.stream(None, &messages, &tools)).await;

    let deltas: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, vec!["streamed ", "answer"]);
    match events.last() {
        Some(StreamEvent::Done(c)) => {
            assert_eq!(c.text.as_deref(), Some("streamed answer"));
            assert!(c.tool_calls.is_empty());
        }
        other => panic!("expected a final Done, got {other:?}"),
    }

    // The request must have asked for streaming.
    let body = server.last_request().json();
    assert_eq!(
        body["stream"], true,
        "stream() must set stream:true: {body}"
    );
}

#[tokio::test]
async fn openai_stream_delivers_text_deltas_and_a_final_completion() {
    let server = Server::start(vec![sse_reply(OPENAI_SSE)]).await;
    let provider = OpenAiCompat::new(&openai_cfg(&server.url())).unwrap();
    let messages = user("hi");
    let tools: Vec<Value> = Vec::new();

    let events = collect(provider.stream(None, &messages, &tools)).await;
    let deltas: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, vec!["streamed ", "answer"]);
    match events.last() {
        Some(StreamEvent::Done(c)) => assert_eq!(c.text.as_deref(), Some("streamed answer")),
        other => panic!("expected a final Done, got {other:?}"),
    }
    assert_eq!(server.last_request().json()["stream"], true);
}

/// An HTTP error on the streaming path must arrive as the first stream event
/// rather than a panic or a silently empty stream.
#[tokio::test]
async fn a_streaming_request_that_fails_surfaces_the_error_as_an_event() {
    let server = Server::start(vec![json_reply(
        503,
        r#"{"error":"provider is overloaded"}"#,
    )])
    .await;
    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let messages = user("hi");
    let tools: Vec<Value> = Vec::new();

    let mut stream = provider.stream(None, &messages, &tools);
    let first = tokio::time::timeout(Duration::from_secs(20), stream.next())
        .await
        .expect("stream should not hang on an HTTP error")
        .expect("the error must be delivered as an event");
    let err = first.expect_err("a 503 must be an error event");
    let msg = format!("{err:#}");
    assert!(msg.contains("503"), "error: {msg}");
}

/// A body that stops mid-stream (proxy reset, provider crash) must surface as
/// a stream error, not a hang and not a truncated "successful" answer.
#[tokio::test]
async fn a_body_that_stops_mid_stream_surfaces_a_stream_error() {
    let partial = "event: message_start\ndata: {\"type\":\"message_start\"}\n\n";
    let server = Server::start(vec![Reply::Truncated {
        declared_len: partial.len() + 500,
        body: partial.to_string(),
    }])
    .await;
    let provider = AnthropicMessages::new(&anthropic_cfg(&server.url())).unwrap();
    let messages = user("hi");
    let tools: Vec<Value> = Vec::new();

    let mut stream = provider.stream(None, &messages, &tools);
    let mut saw_error = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_secs(20), stream.next()).await {
            Ok(Some(Err(_))) => {
                saw_error = true;
                break;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(None) => break,
            Err(_) => panic!("stream hung after a truncated body"),
        }
    }
    assert!(
        saw_error,
        "a connection that dies mid-body must produce an error event"
    );
}

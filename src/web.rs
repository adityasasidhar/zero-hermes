//! Local web UI: axum + SSE, a single-page chat console.
//!
//! Architecture:
//!
//! - One in-process [`tokio::sync::broadcast`] channel of [`UiEvent`]s.
//! - `POST /send` runs the agent loop for one user message; events are sent
//!   on the broadcast channel as they happen.
//! - `GET /events` is an SSE endpoint that subscribes to the broadcast and
//!   streams events to the browser.
//! - `GET /` serves the static `index.html`.
//!
//! The UI is one HTML file — no framework, no bundler, no build step — but a
//! complete console: it submits with `fetch` so a turn never navigates away,
//! renders streaming markdown, shows tool calls as collapsible cards, and
//! keeps session/connection state in a sidebar. `/send` still accepts a plain
//! form POST, so the page degrades without JavaScript.
//!
//! The page's script escapes all model and tool output and then validates the
//! parsed nodes against an allowlist before they reach the live DOM (see
//! `setMd`/`sanitize` in `web/index.html`). The `{{CSRF_TOKEN}}` placeholder is
//! substituted by [`serve_index`].

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{
    sse::{Event, Sse},
    Html, IntoResponse, Redirect, Response,
};
use axum::routing::{get, post};
use axum::{Form, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};

use crate::agent::{run_stream, LlmProvider, Message, RunLimits, StreamTurn, ToolContext};
use crate::error::Result;
use crate::memory::Memory;
use crate::tools::ToolRegistry;

/// Events the web UI cares about. Anything an agent does that's user-visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// A user turn was submitted (echoed back so the UI can append it
    /// immediately rather than waiting for the agent).
    User { text: String },
    /// A chunk of assistant text arrived.
    TextDelta { delta: String },
    /// The assistant emitted a tool call request.
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    /// A tool call completed (with its output).
    ToolResult {
        name: String,
        output: String,
        is_error: bool,
    },
    /// The agent loop finished for this turn.
    Done,
    /// The agent loop failed.
    Error { message: String },
    /// The broadcast channel lagged: a slow client missed `missed` events.
    /// Surfaced once per lag so the UI can warn the user.
    Lagged { missed: u64 },
}

/// Shared state for the web server.
#[derive(Clone)]
pub struct AppState {
    /// Broadcast channel that every SSE connection subscribes to.
    pub events: broadcast::Sender<UiEvent>,
    /// The conversation history (one in-memory session).
    ///
    /// `tokio::sync::Mutex` (not `RwLock`): we always write, and the lock
    /// is held across the entire agent run. There's no concurrent reader
    /// that would justify the read-side fast path.
    pub history: Arc<Mutex<Vec<Message>>>,
    /// System prompt (built from skills + provider.system).
    pub system: String,
    /// Provider for the agent loop.
    pub provider: Arc<dyn LlmProvider>,
    /// Tool registry.
    pub tools: Arc<ToolRegistry>,
    /// Memory handle (for tools that read/write notes).
    pub memory: Arc<Memory>,
    /// Per-turn iteration and context-window limits.
    pub limits: RunLimits,
    /// Per-process CSRF token.
    ///
    /// `POST /send` is a form submission, which browsers treat as a simple
    /// request: no preflight, and any page on the internet can send one to
    /// `localhost`. Since the agent has a `bash` tool, an unprotected
    /// endpoint is drive-by code execution. The token is minted at startup
    /// and injected into the page, so only a document served by this
    /// process can submit.
    pub csrf_token: String,
}

impl AppState {
    /// Broadcast a UI event to every connected SSE client. Errors are
    /// ignored — if no client is listening, the event is dropped.
    pub fn emit(&self, ev: UiEvent) {
        let _ = self.events.send(ev);
    }
}

/// Build the axum router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/events", get(sse_handler))
        .route("/send", post(send_handler))
        .route("/health", get(health_handler))
        .with_state(state)
}

/// The page itself — vanilla DOM, no dependencies. See the `index.html`
/// header/section comments for the structure of the client.
const INDEX_HTML: &str = include_str!("../web/index.html");

/// Placeholder in `web/index.html` replaced with the live CSRF token.
const CSRF_PLACEHOLDER: &str = "{{CSRF_TOKEN}}";

async fn serve_index(State(state): State<AppState>) -> Html<String> {
    Html(INDEX_HTML.replace(CSRF_PLACEHOLDER, &state.csrf_token))
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

#[derive(Debug, Deserialize)]
struct SendForm {
    message: String,
    /// Must match [`AppState::csrf_token`]. See the field docs for why.
    #[serde(default)]
    csrf: String,
}

/// Constant-time-ish string comparison. Not a defence against a remote
/// timing attack over a network this coarse, but it costs nothing and
/// keeps the check from short-circuiting on the first byte.
fn tokens_match(a: &str, b: &str) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// POST /send — kick off one agent turn for the submitted message.
/// Streams events on the broadcast channel as the loop runs.
async fn send_handler(State(state): State<AppState>, Form(form): Form<SendForm>) -> Response {
    // Reject anything that did not come from a page this process served.
    // A form POST is a "simple request": no CORS preflight, so without
    // this any site the user visits could drive the `bash` tool on their
    // machine just by targeting localhost.
    if !tokens_match(&form.csrf, &state.csrf_token) {
        tracing::warn!("rejected /send with a missing or invalid CSRF token");
        return (StatusCode::FORBIDDEN, "invalid or missing CSRF token").into_response();
    }
    let message = form.message.trim().to_string();
    if message.is_empty() {
        return Redirect::to("/").into_response();
    }
    // Echo the user message immediately so the UI shows it before the agent
    // loop starts (which can take a few hundred ms for the LLM round-trip).
    state.emit(UiEvent::User {
        text: message.clone(),
    });

    let history = state.history.clone();
    let provider = state.provider.clone();
    let tools = state.tools.clone();
    let memory = state.memory.clone();
    let system = state.system.clone();
    let limits = state.limits;
    let state_for_loop = state.clone();

    // Spawn the agent loop on its own task so the HTTP handler can return
    // immediately and the browser can keep the SSE stream open to receive
    // events as they happen.
    tokio::spawn(async move {
        let ctx = ToolContext {
            cwd: None,
            session_id: None,
            memory: Some(memory),
        };
        // Hold the history mutex for the duration of the agent run. This
        // serializes turns (the previous RwLock was misleading — we
        // always wrote).
        let mut history_guard = history.lock().await;
        let result = run_stream(
            provider.as_ref(),
            tools.as_ref(),
            Some(&system),
            &mut history_guard,
            &message,
            limits,
            &ctx,
            |ev| match ev {
                StreamTurn::TextDelta(s) => {
                    state_for_loop.emit(UiEvent::TextDelta { delta: s });
                }
                StreamTurn::ToolUse(tc) => {
                    state_for_loop.emit(UiEvent::ToolUse {
                        name: tc.name,
                        input: tc.input,
                    });
                }
                StreamTurn::ToolResult {
                    name,
                    output,
                    is_error,
                } => {
                    state_for_loop.emit(UiEvent::ToolResult {
                        name,
                        output,
                        is_error,
                    });
                }
                StreamTurn::Done(_) => {
                    state_for_loop.emit(UiEvent::Done);
                }
            },
        )
        .await;
        if let Err(e) = result {
            state_for_loop.emit(UiEvent::Error {
                message: e.to_string(),
            });
        }
    });

    Redirect::to("/").into_response()
}

/// SSE handler — subscribes to the broadcast channel and yields events
/// formatted as `data: {...}\n\n`. Uses `tokio_stream::BroadcastStream`
/// to bridge the broadcast receiver to a poll-based stream.
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures::Stream<Item = std::result::Result<Event, axum::Error>>> {
    let rx = state.events.subscribe();
    let mut inner = tokio_stream::wrappers::BroadcastStream::new(rx);
    let stream = async_stream::stream! {
        while let Some(item) = inner.next().await {
            let ev = match item {
                Ok(ev) => ev,
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                    // A slow client missed events. Surface a one-off
                    // `Lagged` UI event so the browser can warn the user
                    // that some output is gone.
                    let json = serde_json::to_string(&UiEvent::Lagged { missed: n })
                        .unwrap_or_else(|_| "{}".into());
                    yield Ok(Event::default().data(json));
                    continue;
                }
            };
            let json = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".into());
            yield Ok(Event::default().data(json));
        }
    };
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

/// Bind address for the web server.
#[derive(Debug, Clone)]
pub struct BindAddr {
    pub host: String,
    pub port: u16,
}

impl Default for BindAddr {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8088,
        }
    }
}

/// Parse a host:port string into a `BindAddr`. Falls back to default on
/// malformed input.
pub fn parse_bind(s: &str) -> BindAddr {
    match s.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => BindAddr {
                host: host.to_string(),
                port,
            },
            Err(_) => BindAddr::default(),
        },
        None => BindAddr::default(),
    }
}

/// Run the web server. Blocks until the server is shut down.
pub async fn serve(state: AppState, bind: BindAddr) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", bind.host, bind.port)
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address: {e}"))?;
    if !addr.ip().is_loopback() {
        tracing::warn!(
            %addr,
            "binding outside loopback: the agent can run shell commands, so              anyone who can reach this port can run them on this host"
        );
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "zero-hermes web UI listening");
    let app = router(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| anyhow::anyhow!("axum serve: {e}"))?;
    Ok(())
}

/// Resolve on Ctrl-C so in-flight responses can finish instead of being
/// cut off mid-stream.
async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("shutdown signal received"),
        Err(e) => tracing::warn!(error = %e, "failed to listen for ctrl-c"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bind_valid() {
        let b = parse_bind("0.0.0.0:9000");
        assert_eq!(b.port, 9000);
        assert_eq!(b.host, "0.0.0.0");
    }

    #[test]
    fn parse_bind_fallback() {
        assert_eq!(parse_bind("garbage").port, 8088);
        assert_eq!(parse_bind("").port, 8088);
        assert_eq!(parse_bind("host:badport").port, 8088);
    }

    #[test]
    fn ui_event_round_trip_text_delta() {
        let ev = UiEvent::TextDelta { delta: "hi".into() };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"text_delta\""));
        assert!(s.contains("\"hi\""));
    }

    #[test]
    fn ui_event_round_trip_tool_use() {
        let ev = UiEvent::ToolUse {
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"tool_use\""));
        assert!(s.contains("\"bash\""));
        assert!(s.contains("\"command\""));
    }

    #[test]
    fn ui_event_round_trip_done() {
        let ev = UiEvent::Done;
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"done\""));
    }
}

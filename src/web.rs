//! Minimal web UI: axum + SSE, single-page app with a textarea input and a
//! streaming message list.
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
//! The UI is intentionally tiny — one HTML file, ~30 lines of vanilla JS
//! using `EventSource` to render streaming events. No framework, no bundler.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{
    sse::{Event, Sse},
    Html, IntoResponse, Redirect,
};
use axum::routing::{get, post};
use axum::{Form, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::agent::{run_stream, LlmProvider, Message, StreamTurn, ToolContext};
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
}

/// Shared state for the web server.
#[derive(Clone)]
pub struct AppState {
    /// Broadcast channel that every SSE connection subscribes to.
    pub events: broadcast::Sender<UiEvent>,
    /// The conversation history (one in-memory session).
    pub history: Arc<RwLock<Vec<Message>>>,
    /// System prompt (built from skills + provider.system).
    pub system: String,
    /// Provider for the agent loop.
    pub provider: Arc<dyn LlmProvider>,
    /// Tool registry.
    pub tools: Arc<ToolRegistry>,
    /// Memory handle (for tools that read/write notes).
    pub memory: Arc<Memory>,
    /// Max iterations per turn.
    pub max_iterations: usize,
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

/// The page itself — vanilla DOM, one tiny script for EventSource.
const INDEX_HTML: &str = include_str!("../web/index.html");

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

#[derive(Debug, Deserialize)]
struct SendForm {
    message: String,
}

/// POST /send — kick off one agent turn for the submitted message.
/// Streams events on the broadcast channel as the loop runs.
async fn send_handler(State(state): State<AppState>, Form(form): Form<SendForm>) -> Redirect {
    let message = form.message.trim().to_string();
    if message.is_empty() {
        return Redirect::to("/");
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
    let max_iterations = state.max_iterations;
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
        let mut history_guard = history.write().await;
        let result = run_stream(
            provider.as_ref(),
            tools.as_ref(),
            Some(&system),
            &mut history_guard,
            &message,
            max_iterations,
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

    Redirect::to("/")
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
            // `BroadcastStream` only yields `Ok` or `Lagged`; the `Lagged`
            // variant means a slow client missed events — drop them silently
            // and keep streaming.
            if let Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) = item {
                continue;
            }
            let ev = item.expect("BroadcastStream only yields Ok | Lagged");
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
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "zero-hermes web UI listening");
    let app = router(state);
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("axum serve: {e}"))?;
    Ok(())
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

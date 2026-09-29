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
//! - `GET /assets/…` serves the embedded Hermes mark — one static route per
//!   entry in [`ASSETS`], not a catch-all.
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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{
    sse::{Event, Sse},
    Html, IntoResponse, Redirect, Response,
};
use axum::routing::{delete, get, post};
use axum::{Form, Json, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};

use crate::agent::{run_stream, LlmProvider, Message, RunLimits, StreamTurn, ToolContext};
use crate::error::Result;
use crate::memory::{recall_section, Memory};
use crate::tools::ToolRegistry;

/// Durable session id for the single shared web conversation. The web UI
/// keeps one in-memory history like the CLI chat, so it gets one stable
/// session row to append to.
pub const WEB_SESSION_ID: &str = "web-default";

/// Events the web UI cares about. Anything an agent does that's user-visible.
///
/// Every turn-scoped variant carries the owning `session` id (`web-*`) so a
/// browser with several conversations open can ignore traffic for background
/// sessions instead of interleaving every turn into the active view.
/// `Lagged` stays global: it reports transport loss on the shared broadcast,
/// not one turn's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// A user turn was submitted (echoed back so the UI can append it
    /// immediately rather than waiting for the agent).
    User { session: String, text: String },
    /// A chunk of assistant text arrived.
    TextDelta { session: String, delta: String },
    /// The assistant emitted a tool call request.
    ToolUse {
        session: String,
        name: String,
        input: serde_json::Value,
    },
    /// A tool call completed (with its output).
    ToolResult {
        session: String,
        name: String,
        output: String,
        is_error: bool,
    },
    /// The agent loop finished for this turn.
    Done { session: String },
    /// The agent loop failed.
    Error { session: String, message: String },
    /// The broadcast channel lagged: a slow client missed `missed` events.
    /// Surfaced once per lag so the UI can warn the user.
    Lagged { missed: u64 },
}

impl UiEvent {
    /// Owning session for turn-scoped events; `None` for transport-level
    /// `Lagged`, which belongs to no single conversation.
    pub fn session(&self) -> Option<&str> {
        match self {
            UiEvent::User { session, .. }
            | UiEvent::TextDelta { session, .. }
            | UiEvent::ToolUse { session, .. }
            | UiEvent::ToolResult { session, .. }
            | UiEvent::Done { session }
            | UiEvent::Error { session, .. } => Some(session),
            UiEvent::Lagged { .. } => None,
        }
    }
}

/// Per-session turn locks: same session serializes, different sessions stay
/// concurrent. See [`session_turn_lock`].
pub type SessionLocks = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;

/// Shared state for the web server.
#[derive(Clone)]
pub struct AppState {
    /// Broadcast channel that every SSE connection subscribes to.
    pub events: broadcast::Sender<UiEvent>,
    /// Per-session conversation histories, keyed by `web-*` session id.
    ///
    /// `tokio::sync::Mutex` (not `RwLock`): turns clone a snapshot, run
    /// without the lock, then store back. Same-session turns are serialized
    /// via [`AppState::session_locks`] so concurrent posts to one session
    /// cannot interleave positions (last-writer-wins only across *different*
    /// sessions, which stay concurrent). Each turn appends its delta via
    /// [`Memory::append_messages`] so a restart resumes the transcript.
    /// The SSE broadcast stays global: every browser tab receives every
    /// turn's events, each tagged with its `session`, and the page filters
    /// to the active conversation.
    pub histories: Arc<Mutex<HashMap<String, Vec<Message>>>>,
    /// Per-session turn locks: same session serializes, different sessions
    /// stay concurrent. See [`session_turn_lock`].
    pub session_locks: SessionLocks,
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

/// Acquire (creating on first use) the per-session turn lock.
///
/// Same session serializes; different sessions stay concurrent. The
/// returned `Arc` is held across the whole turn (snapshot → LLM → store)
/// so two posts to one session cannot duplicate positions.
pub async fn session_turn_lock(map: &SessionLocks, session_id: &str) -> Arc<Mutex<()>> {
    let mut guard = map.lock().await;
    guard
        .entry(session_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Build the axum router.
pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/", get(serve_index))
        .route("/events", get(sse_handler))
        .route("/send", post(send_handler))
        .route("/health", get(health_handler))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/:id/history", get(session_history_handler))
        .route("/api/sessions/:id", delete(delete_session_handler));
    // One static route per embedded image rather than a `/assets/{*path}`
    // catch-all: the asset set is a closed literal, so an exact match is all
    // that is ever needed, and a request for anything else falls through to
    // axum's 404 without ever naming a file.
    for (path, _) in ASSETS {
        app = app.route(path, get(asset_handler));
    }
    app.with_state(state)
}

/// The page itself — vanilla DOM, no dependencies. See the `index.html`
/// header/section comments for the structure of the client.
const INDEX_HTML: &str = include_str!("../web/index.html");

/// Placeholder in `web/index.html` replaced with the live CSRF token.
const CSRF_PLACEHOLDER: &str = "{{CSRF_TOKEN}}";

/// The Hermes mark, greyscaled and head-cropped to a square so it still reads
/// as a face at favicon size. Embedded rather than read from disk: the binary
/// has to stay self-contained, and this is how `index.html` already ships.
const LOGO_MARK: &[u8] = include_bytes!("../assets/hermes-mark.png");
const LOGO_FAVICON: &[u8] = include_bytes!("../assets/hermes-favicon.png");

/// Every embedded image, keyed by the exact public path it is served at. The
/// path doubles as the routing key, so listing an image here is all it takes
/// to publish it.
const ASSETS: &[(&str, &[u8])] = &[
    ("/assets/hermes-mark.png", LOGO_MARK),
    ("/assets/hermes-favicon.png", LOGO_FAVICON),
];

async fn serve_index(State(state): State<AppState>) -> Html<String> {
    Html(INDEX_HTML.replace(CSRF_PLACEHOLDER, &state.csrf_token))
}

/// Serve one of the embedded images. Each is registered under its own exact
/// path, so the lookup is a scan of a two-entry literal and an unknown asset
/// never reaches here.
async fn asset_handler(uri: Uri) -> Response {
    match ASSETS.iter().find(|(path, _)| *path == uri.path()) {
        Some((_, bytes)) => png_response(bytes),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Wrap embedded image bytes in a response. The PNG signature is a cheap guard
/// against wiring the wrong file to a path. Immutable per process, so a day
/// of caching is safe and keeps repeat page loads off the wire.
///
/// `'static` because [`ASSETS`] holds `&'static [u8]`, so every byte served
/// comes from the binary rather than a caller.
fn png_response(bytes: &'static [u8]) -> Response {
    const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if !bytes.starts_with(PNG_MAGIC) {
        tracing::warn!("embedded asset is not a PNG; serving it as one would be a lie");
        return (StatusCode::INTERNAL_SERVER_ERROR, "corrupt asset").into_response();
    }
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
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
    /// Optional session key; defaults to `web-default`. Accepts a bare key
    /// (`alice` -> `web-alice`) or a full `web-*` id.
    #[serde(default)]
    session: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SessionQuery {
    #[serde(default)]
    session: Option<String>,
}

/// Normalize a user-supplied web session into a `web-*` session id.
pub fn normalize_web_session(raw: Option<&str>) -> String {
    let s = raw.unwrap_or("").trim();
    if s.is_empty() {
        return "web-default".to_string();
    }
    let mut clean: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    clean.truncate(64);
    if clean.is_empty() || clean == "web-" {
        return "web-default".to_string();
    }
    if clean.starts_with("web-") {
        clean
    } else {
        format!("web-{clean}")
    }
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

/// One row in the session picker: the durable id plus enough transcript
/// shape to render a useful label without loading every history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOverview {
    pub id: String,
    pub message_count: usize,
    pub preview: Option<String>,
    pub created_at: String,
}

/// `POST /api/sessions` body. `name` is optional: omitted means
/// "mint me a fresh id". `csrf` follows the same per-process token rule
/// as `/send` — the agent has a shell tool, so JSON endpoints that mutate
/// state need it too. Browsers can also send it as `x-csrf-token`.
#[derive(Debug, Deserialize, Default)]
struct CreateSessionRequest {
    #[serde(default)]
    csrf: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    session: SessionOverview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
struct HistoryResponse {
    session: String,
    messages: Vec<HistoryMessage>,
}

/// Short chat id for a `web-*` session id (`web-alice` -> `alice`).
fn web_chat_id(session_id: &str) -> &str {
    session_id.strip_prefix("web-").unwrap_or(session_id)
}

/// Pull the caller's CSRF token from a JSON body field or the
/// `x-csrf-token` header (the page sends the header; curl sends either).
fn request_csrf(body_csrf: Option<&str>, headers: &HeaderMap) -> String {
    if let Some(v) = body_csrf {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    headers
        .get("x-csrf-token")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// Build one picker row: durable count + preview, preferring the live
/// in-memory history (which includes the in-flight turn) over SQLite.
async fn session_overview(
    state: &AppState,
    session_id: &str,
    created_at: String,
) -> SessionOverview {
    let (count, preview) = {
        let guard = state.histories.lock().await;
        if let Some(h) = guard.get(session_id) {
            let preview = h
                .iter()
                .rev()
                .map(|m| m.text())
                .find(|t| !t.trim().is_empty())
                .map(|t| crate::util::truncate_bytes(t.trim(), 120));
            (h.len(), preview)
        } else {
            drop(guard);
            let count = state.memory.session_message_count(session_id).unwrap_or(0);
            let preview = state.memory.session_last_text(session_id).unwrap_or(None);
            (count, preview)
        }
    };
    SessionOverview {
        id: session_id.to_string(),
        message_count: count,
        preview,
        created_at,
    }
}

/// GET /api/sessions — every `web-*` conversation, newest first.
///
/// Merges durable SQLite rows with live in-memory histories so a session
/// created this process but not yet turned still shows up. The picker
/// polls this after every completed turn to refresh counts/previews.
async fn list_sessions(State(state): State<AppState>) -> Response {
    let mut by_id: HashMap<String, String> = HashMap::new();
    if let Ok(rows) = state.memory.list_sessions_limited(200) {
        for row in rows {
            if row.channel == "web" || row.id.starts_with("web-") {
                by_id
                    .entry(row.id.clone())
                    .or_insert(row.created_at.clone());
            }
        }
    }
    {
        let guard = state.histories.lock().await;
        for id in guard.keys() {
            if id.starts_with("web-") && !by_id.contains_key(id) {
                by_id.insert(id.clone(), String::new());
            }
        }
        // Guarantee the default exists so a fresh profile still renders one row.
        if !by_id.contains_key(WEB_SESSION_ID) {
            by_id.insert(WEB_SESSION_ID.to_string(), String::new());
        }
    }
    let mut out = Vec::with_capacity(by_id.len());
    for (id, created_at) in by_id {
        out.push(session_overview(&state, &id, created_at).await);
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    Json(out).into_response()
}

/// POST /api/sessions — create (or ensure) one `web-*` conversation.
///
/// An explicit `name` is normalized (`alice` -> `web-alice`); omitted means
/// a fresh `web-<hex>` id. Idempotent: re-posting an existing name returns
/// its current overview instead of duplicating it.
async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionRequest>,
) -> Response {
    let csrf = request_csrf(body.csrf.as_deref(), &headers);
    if !tokens_match(&csrf, &state.csrf_token) {
        return (StatusCode::FORBIDDEN, "invalid or missing CSRF token").into_response();
    }
    let session_id = match body.name {
        Some(name) if !name.trim().is_empty() => normalize_web_session(Some(&name)),
        _ => format!("web-{}", crate::util::random_hex(4)),
    };
    if let Err(e) = state
        .memory
        .upsert_session(&session_id, "web", web_chat_id(&session_id))
    {
        tracing::warn!(error = %e, "creating web session failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not create session",
        )
            .into_response();
    }
    {
        let mut guard = state.histories.lock().await;
        guard.entry(session_id.clone()).or_insert_with(Vec::new);
    }
    let created_at = state
        .memory
        .list_sessions_limited(200)
        .ok()
        .and_then(|rows| {
            rows.into_iter()
                .find(|r| r.id == session_id)
                .map(|r| r.created_at)
        })
        .unwrap_or_default();
    let overview = session_overview(&state, &session_id, created_at).await;
    (
        StatusCode::CREATED,
        Json(CreateSessionResponse { session: overview }),
    )
        .into_response()
}

/// GET /api/sessions/:id/history — durable + live transcript as
/// `{role, text}` pairs, skipping empty tool-only chatter the picker
/// cannot render. Tool-use/result blocks surface via their text payload
/// (results) or are dropped (bare tool_use has no text).
async fn session_history_handler(
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
) -> Response {
    let session_id = normalize_web_session(Some(&raw_id));
    let history = {
        let guard = state.histories.lock().await;
        if let Some(h) = guard.get(&session_id) {
            h.clone()
        } else {
            drop(guard);
            match state.memory.load_history(&session_id) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(error = %e, "loading session history failed");
                    return (StatusCode::INTERNAL_SERVER_ERROR, "could not load history")
                        .into_response();
                }
            }
        }
    };
    let messages: Vec<HistoryMessage> = history
        .iter()
        .filter_map(|m| {
            let text = m.text();
            if m.role == "user"
                && text.trim().is_empty()
                && m.content
                    .iter()
                    .any(|c| matches!(c, crate::agent::ContentBlock::ToolUse(_)))
            {
                // Assistant tool_use blocks live in role=assistant messages,
                // which have no text — drop them, the turn's text/tool cards
                // stream separately. User tool_result blocks DO have text.
                return None;
            }
            if text.trim().is_empty() {
                return None;
            }
            let role = if m.role == "assistant" || m.role == "user" {
                m.role.clone()
            } else {
                "system".to_string()
            };
            Some(HistoryMessage { role, text })
        })
        .collect();
    Json(HistoryResponse {
        session: session_id,
        messages,
    })
    .into_response()
}

/// DELETE /api/sessions/:id — drop the transcript and the session row.
///
/// Idempotent: unknown ids still return 200. The CSRF token arrives via
/// `x-csrf-token` (page) or `?csrf=` (curl); either is accepted.
async fn delete_session_handler(
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let csrf = request_csrf(q.get("csrf").map(String::as_str), &headers);
    if !tokens_match(&csrf, &state.csrf_token) {
        return (StatusCode::FORBIDDEN, "invalid or missing CSRF token").into_response();
    }
    let session_id = normalize_web_session(Some(&raw_id));
    if session_id == WEB_SESSION_ID {
        // The default conversation is the picker's anchor: clear its
        // transcript but keep the row so the list never renders empty.
        if let Err(e) = state.memory.save_history(&session_id, &[]) {
            tracing::warn!(error = %e, "clearing default session failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "could not clear session").into_response();
        }
        let mut guard = state.histories.lock().await;
        guard.insert(session_id, Vec::new());
        return Json(serde_json::json!({"ok": true})).into_response();
    }
    if let Err(e) = state.memory.delete_session(&session_id) {
        tracing::warn!(error = %e, "deleting session failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not delete session",
        )
            .into_response();
    }
    {
        let mut guard = state.histories.lock().await;
        guard.remove(&session_id);
    }
    Json(serde_json::json!({"ok": true})).into_response()
}

/// POST /send — kick off one agent turn for the submitted message.
/// Streams events on the broadcast channel as the loop runs.
async fn send_handler(
    State(state): State<AppState>,
    Query(query): Query<SessionQuery>,
    Form(form): Form<SendForm>,
) -> Response {
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
    let session_id = normalize_web_session(form.session.as_deref().or(query.session.as_deref()));
    // Ensure the durable row exists before the first turn: ad-hoc web
    // sessions (picker-created or hand-typed) otherwise appear in the
    // picker only after their turn persists.
    if let Err(e) = state
        .memory
        .upsert_session(&session_id, "web", web_chat_id(&session_id))
    {
        tracing::warn!(error = %e, "upserting web session failed");
    }
    // Echo the user message immediately so the UI shows it before the agent
    // loop starts (which can take a few hundred ms for the LLM round-trip).
    state.emit(UiEvent::User {
        session: session_id.clone(),
        text: message.clone(),
    });

    let histories = state.histories.clone();
    let session_locks = state.session_locks.clone();
    let provider = state.provider.clone();
    let tools = state.tools.clone();
    let memory = state.memory.clone();
    let system = state.system.clone();
    let limits = state.limits;
    let state_for_loop = state.clone();

    // Spawn the agent loop on its own task so the HTTP handler can return
    // immediately and the browser can keep the SSE stream open to receive
    // events as they happen. Same-session turns serialize on the
    // per-session lock; different sessions stay concurrent. The turn
    // snapshots history, runs without the histories lock, then stores back
    // and appends the delta to SQLite. The `message` tool outbox is
    // gateway-only (n/a here): this single UI already streams the turn.
    tokio::spawn(async move {
        let turn_lock = session_turn_lock(&session_locks, &session_id).await;
        let _turn_guard = turn_lock.lock().await;
        let snapshot = {
            let guard = histories.lock().await;
            if let Some(h) = guard.get(&session_id) {
                h.clone()
            } else {
                drop(guard);
                let loaded = memory.load_history(&session_id).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "loading web session history failed");
                    Vec::new()
                });
                let mut guard = histories.lock().await;
                guard.entry(session_id.clone()).or_insert_with(Vec::new);
                loaded
            }
        };
        let mut history = snapshot;
        let ctx = ToolContext {
            cwd: None,
            session_id: Some(session_id.clone()),
            memory: Some(memory.clone()),
        };
        // Recall is injected every turn so the agent sees past context
        // without having to ask via the memory tool first. The current
        // session is excluded so recall surfaces *other* conversations
        // instead of echoing back what was just said.
        let effective_system = match recall_section(&memory, &message, Some(session_id.as_str()), 4)
        {
            Some(section) => format!("{system}\n\n{section}"),
            None => system,
        };
        let len_before = history.len();
        let result = run_stream(
            provider.as_ref(),
            tools.as_ref(),
            Some(&effective_system),
            &mut history,
            &message,
            limits,
            &ctx,
            |ev| match ev {
                StreamTurn::TextDelta(s) => {
                    state_for_loop.emit(UiEvent::TextDelta {
                        session: session_id.clone(),
                        delta: s,
                    });
                }
                StreamTurn::ToolUse(tc) => {
                    state_for_loop.emit(UiEvent::ToolUse {
                        session: session_id.clone(),
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
                        session: session_id.clone(),
                        name,
                        output,
                        is_error,
                    });
                }
                StreamTurn::Done(_) => {
                    state_for_loop.emit(UiEvent::Done {
                        session: session_id.clone(),
                    });
                }
            },
        )
        .await;
        match result {
            Ok(_) => {
                {
                    let mut guard = histories.lock().await;
                    guard.insert(session_id.clone(), history.clone());
                }
                // Append only the delta: the agent loop compacts the
                // in-memory history in place, so persisting the whole vector
                // would delete the durable prefix.
                let start = len_before.min(history.len());
                if let Err(e) = memory.append_messages(&session_id, &history[start..]) {
                    tracing::warn!(error = %e, "saving web session history failed");
                }
            }
            Err(e) => {
                state_for_loop.emit(UiEvent::Error {
                    session: session_id.clone(),
                    message: e.to_string(),
                });
                // `run_stream` pushes the user message before the first LLM
                // call, so a failure leaves an orphan trailing user turn.
                // Pop it so the next turn doesn't start with `user,user`,
                // then persist the repaired history.
                crate::agent::repair_history_after_failure(&mut history);
                {
                    let mut guard = histories.lock().await;
                    guard.insert(session_id.clone(), history.clone());
                }
                let start = len_before.min(history.len());
                if let Err(e) = memory.append_messages(&session_id, &history[start..]) {
                    tracing::warn!(error = %e, "saving web session history after failure failed");
                }
            }
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
        let ev = UiEvent::TextDelta {
            session: "web-a".into(),
            delta: "hi".into(),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"text_delta\""));
        assert!(s.contains("\"hi\""));
        assert!(s.contains("web-a"));
        assert_eq!(ev.session(), Some("web-a"));
    }

    #[test]
    fn ui_event_round_trip_tool_use() {
        let ev = UiEvent::ToolUse {
            session: "web-a".into(),
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
        let ev = UiEvent::Done {
            session: "web-a".into(),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"done\""));
        assert!(s.contains("web-a"));
    }

    #[test]
    fn lagged_has_no_session() {
        assert_eq!(UiEvent::Lagged { missed: 2 }.session(), None);
    }

    #[test]
    fn web_chat_id_strips_prefix() {
        assert_eq!(web_chat_id("web-alice"), "alice");
        assert_eq!(web_chat_id("web-default"), "default");
    }
}

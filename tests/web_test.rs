//! Smoke tests for the web UI: page serves, /send routes, /events streams.
//!
//! These tests use axum's `Router` directly with `axum::body::to_bytes` and
//! `tower::ServiceExt::oneshot` — no real socket needed.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use futures::StreamExt;
use http_body_util::BodyStream;
use tokio::sync::Mutex;
use tower::ServiceExt;

use zero_hermes::agent::ToolContext;
use zero_hermes::memory::Memory;
use zero_hermes::tools::ToolRegistry;
use zero_hermes::web::{normalize_web_session, router, AppState, UiEvent};

/// Build a minimal `AppState` for tests — no network calls, in-memory
/// everything.
fn test_state() -> AppState {
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    AppState {
        events: tx,
        histories: Arc::new(Mutex::new(std::collections::HashMap::new())),
        session_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        system: "you are zero-hermes".into(),
        provider: Arc::new(zero_hermes::agent::mock::MockProvider::text_only("hi")),
        tools: Arc::new(ToolRegistry::new()),
        memory: Arc::new(Memory::in_memory().unwrap()),
        limits: zero_hermes::agent::RunLimits::iterations(3),
        csrf_token: "test-token".to_string(),
    }
}

#[tokio::test]
async fn index_html_serves() {
    let app = router(test_state());
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("<textarea"), "missing textarea: {html}");
    assert!(
        html.contains("EventSource"),
        "missing EventSource wiring: {html}"
    );
    assert!(html.contains("zero-hermes"));
}

#[tokio::test]
async fn health_endpoint() {
    let app = router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(body.as_ref(), b"ok");
}

#[tokio::test]
async fn send_routes_with_empty_message() {
    // Empty message should bounce back to "/" via redirect, not kick off the
    // agent loop. We don't follow redirects — we just check the status.
    let app = router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("csrf=test-token&message="))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let loc = response
        .headers()
        .get("location")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    assert_eq!(loc, "/");
}

#[tokio::test]
async fn send_with_message_returns_redirect_and_emits_user_event() {
    // Subscribe before the handler runs so we don't miss the User event.
    let state = test_state();
    let mut rx = state.events.subscribe();

    let app = router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("csrf=test-token&message=hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let mut saw_user = false;
    let mut saw_done = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline && !(saw_user && saw_done) {
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(UiEvent::User { text })) => {
                assert_eq!(text, "hello");
                saw_user = true;
            }
            Ok(Ok(UiEvent::Done)) => {
                saw_done = true;
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => continue,
        }
    }
    assert!(saw_user, "expected a User event");
    assert!(saw_done, "expected a Done event");
}

#[tokio::test]
async fn sse_format_is_data_lines() {
    let state = test_state();
    let app = router(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ct = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    assert!(
        ct.starts_with("text/event-stream"),
        "unexpected content-type: {ct}"
    );

    // SSE bodies never close (keep-alive); read with a timeout and accumulate
    // until we see a full SSE frame, or fail after a deadline.
    let mut stream = BodyStream::new(response.into_body());
    let mut buf: Vec<u8> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match tokio::time::timeout(
            timeout.min(std::time::Duration::from_millis(200)),
            stream.next(),
        )
        .await
        {
            Ok(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    buf.extend_from_slice(&data);
                }
            }
            Ok(Some(Err(e))) => panic!("body error: {e}"),
            Ok(None) => break,
            Err(_) => continue,
        }
        let text = String::from_utf8_lossy(&buf);
        if text.contains("\"text_delta\"") && text.contains("\n\n") {
            break;
        }
    }

    // Emit one more event (after subscription) to make sure the connection
    // is alive and reaches the body.
    state.emit(UiEvent::TextDelta {
        delta: "ping".into(),
    });
    // Drain the body for up to one second to capture the new event.
    let drain_until = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while std::time::Instant::now() < drain_until {
        let remaining = drain_until.saturating_duration_since(std::time::Instant::now());
        match tokio::time::timeout(
            remaining.min(std::time::Duration::from_millis(200)),
            stream.next(),
        )
        .await
        {
            Ok(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    buf.extend_from_slice(&data);
                }
            }
            Ok(Some(Err(e))) => panic!("body error: {e}"),
            Ok(None) => break,
            Err(_) => break,
        }
        let text = String::from_utf8_lossy(&buf);
        if text.contains("\"text_delta\"") && text.contains("\"ping\"") {
            break;
        }
    }

    let text = String::from_utf8_lossy(&buf);
    assert!(text.contains("data: "), "missing SSE data: prefix: {text}");
    assert!(
        text.contains("\"text_delta\"") && text.contains("\"ping\""),
        "expected text_delta payload: {text}"
    );
    assert!(text.contains("\n\n"), "SSE frame not terminated: {text}");
}

#[tokio::test]
async fn parse_bind_default_fallback() {
    // Empty / malformed input should resolve to the default port.
    let b = zero_hermes::web::parse_bind("");
    assert_eq!(b.port, 8088);
    assert_eq!(b.host, "127.0.0.1");
    let b = zero_hermes::web::parse_bind("garbage");
    assert_eq!(b.port, 8088);
    let b = zero_hermes::web::parse_bind("0.0.0.0:9000");
    assert_eq!(b.port, 9000);
    assert_eq!(b.host, "0.0.0.0");
}

#[tokio::test]
async fn tool_context_default_is_empty() {
    // Smoke test that the default `ToolContext` we hand to the agent loop
    // does not depend on any external state.
    let ctx = ToolContext::default();
    assert!(ctx.cwd.is_none());
    assert!(ctx.session_id.is_none());
    assert!(ctx.memory.is_none());
}

#[tokio::test]
async fn send_without_csrf_token_is_rejected() {
    // A cross-origin form POST is a "simple request" — no preflight — so
    // without this check any page the user visits could drive the agent's
    // bash tool on localhost.
    let app = router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("message=rm%20-rf%20%2F"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn send_with_wrong_csrf_token_is_rejected() {
    let app = router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("csrf=guessed&message=hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn index_embeds_the_live_csrf_token() {
    let app = router(test_state());
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("value=\"test-token\""),
        "token should be injected into the form"
    );
    assert!(
        !html.contains("{{CSRF_TOKEN}}"),
        "placeholder should be fully substituted"
    );
}

#[test]
fn web_session_normalization() {
    assert_eq!(normalize_web_session(None), "web-default");
    assert_eq!(normalize_web_session(Some("")), "web-default");
    assert_eq!(normalize_web_session(Some("alice")), "web-alice");
    assert_eq!(
        normalize_web_session(Some("web-alice")),
        "web-alice",
        "already-prefixed ids are kept"
    );
}

#[tokio::test]
async fn send_persists_session_to_memory() {
    let state = test_state();
    let memory = state.memory.clone();
    let mut rx = state.events.subscribe();
    let app = router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("csrf=test-token&message=hello+web"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    // Wait for the turn to finish, then poll SQLite until the background
    // task persists the transcript.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(UiEvent::Done)) => break,
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Ok(history) = memory.load_history("web-default") {
            if !history.is_empty() {
                return;
            }
        }
        if std::time::Instant::now() >= deadline {
            panic!("expected web-default session to be persisted to memory");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

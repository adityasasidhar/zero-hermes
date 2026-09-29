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

/// Read buffer for the whole-page assertions below. `index.html` is embedded
/// into the binary as a single file, so "one page" is a fixed ~70 KB blob and
/// this is only a guard against reading something absurd — it is not a budget
/// the UI has to fit in. It used to be 64 KiB, which the page quietly outgrew:
/// the tests then failed with `LengthLimitError` on every full-page read, long
/// before the tokenizer turned the page into a "data" blob in `file(1)`.
const PAGE_READ_LIMIT: usize = 256 * 1024;

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
    let body = to_bytes(response.into_body(), PAGE_READ_LIMIT)
        .await
        .unwrap();
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
            Ok(Ok(UiEvent::User { session, text })) => {
                assert_eq!(text, "hello");
                assert_eq!(session, "web-default");
                saw_user = true;
            }
            Ok(Ok(UiEvent::Done { session })) => {
                assert_eq!(session, "web-default");
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
        session: "web-default".into(),
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
    let body = to_bytes(response.into_body(), PAGE_READ_LIMIT)
        .await
        .unwrap();
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

#[tokio::test]
async fn assets_route_serves_the_logo() {
    let app = router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/hermes-mark.png")
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
    assert_eq!(ct, "image/png");
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    // A real PNG: 8-byte signature, then IHDR.
    assert_eq!(&body[1..4], b"PNG", "not a PNG: {body:?}");
    assert!(body.len() > 100, "suspiciously small logo");
}

#[tokio::test]
async fn assets_route_404s_an_unknown_file() {
    // The asset set is a closed literal, so nothing resolves from disk.
    let app = router(test_state());
    for uri in [
        "/assets/nope.png",
        "/assets/../../../etc/passwd",
        "/assets/hermes-mark.png.bak",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri} should 404");
    }
}

#[tokio::test]
async fn index_references_the_logo_asset() {
    let app = router(test_state());
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), PAGE_READ_LIMIT)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("/assets/hermes-mark.png"),
        "the page should use the Hermes mark"
    );
    assert!(
        html.contains("/assets/hermes-favicon.png"),
        "the page should declare a favicon"
    );
}

#[tokio::test]
async fn index_contains_the_sessions_picker() {
    let app = router(test_state());
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), PAGE_READ_LIMIT)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("id=\"session-list\""),
        "picker container missing"
    );
    assert!(
        html.contains("id=\"btn-new-session\""),
        "new-session button missing"
    );
    assert!(html.contains("id=\"session-pill\""), "session pill missing");
    assert!(
        html.contains("/api/sessions"),
        "page should call the sessions API"
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
            Ok(Ok(UiEvent::Done { .. })) => break,
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

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), PAGE_READ_LIMIT)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn sessions_list_always_contains_the_default() {
    let (status, json) = get_json(router(test_state()), "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = json
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
        .collect();
    assert!(
        ids.contains(&"web-default"),
        "expected web-default in {ids:?}"
    );
}

#[tokio::test]
async fn create_session_mints_an_id_and_is_idempotent() {
    let state = test_state();
    // Explicit name normalizes bare -> web-*.
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-csrf-token", "test-token")
                .body(Body::from(r#"{"name":"alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["session"]["id"], "web-alice");

    // Re-posting the same name returns the same id (no duplicate).
    let (status, json) = get_json(router(state.clone()), "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    let count = json
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("id").and_then(|v| v.as_str()) == Some("web-alice"))
        .count();
    assert_eq!(count, 1, "re-post must not duplicate: {json}");

    // Omitted name mints a fresh web-<hex> id.
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("x-csrf-token", "test-token")
                .body(Body::from(r#"{}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let minted = json["session"]["id"].as_str().unwrap().to_string();
    assert!(
        minted.starts_with("web-") && minted.len() > 5,
        "minted id: {minted}"
    );
}

#[tokio::test]
async fn create_session_rejects_a_bad_csrf_token() {
    let response = router(test_state())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn send_to_a_named_session_scopes_events_and_history() {
    let state = test_state();
    let mut rx = state.events.subscribe();
    let app = router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/send")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("csrf=test-token&message=hi+alice&session=alice"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // The User echo + Done for this turn both carry web-alice.
    let mut saw_user = false;
    let mut saw_done = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline && !(saw_user && saw_done) {
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(UiEvent::User { session, text })) if session == "web-alice" => {
                assert_eq!(text, "hi alice");
                saw_user = true;
            }
            Ok(Ok(UiEvent::Done { session })) if session == "web-alice" => {
                saw_done = true;
            }
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    assert!(saw_user, "expected a web-alice User event");
    assert!(saw_done, "expected a web-alice Done event");

    // History API returns the turn as {role, text} pairs.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let (status, json) =
            get_json(router(state.clone()), "/api/sessions/web-alice/history").await;
        assert_eq!(status, StatusCode::OK);
        let messages = json["messages"].as_array().unwrap();
        if !messages.is_empty() {
            assert_eq!(json["session"], "web-alice");
            assert!(messages
                .iter()
                .any(|m| m["role"] == "user" && m["text"] == "hi alice"));
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!("expected web-alice history to persist");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn delete_session_clears_the_transcript() {
    let state = test_state();
    state
        .memory
        .upsert_session("web-temp", "web", "temp")
        .unwrap();
    state
        .memory
        .save_history(
            "web-temp",
            &[zero_hermes::agent::Message::user("to be deleted")],
        )
        .unwrap();

    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/sessions/web-temp?csrf=test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (status, json) = get_json(router(state.clone()), "/api/sessions/web-temp/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["messages"].as_array().unwrap().is_empty());

    // Unknown ids stay idempotent (200, empty).
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/sessions/web-missing?csrf=test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_session_rejects_a_bad_csrf_token() {
    let response = router(test_state())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/sessions/web-temp")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

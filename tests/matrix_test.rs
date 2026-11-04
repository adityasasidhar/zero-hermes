//! Matrix channel against an in-process mock homeserver.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zero_hermes::channels::matrix::{MatrixChannel, MatrixConfig};
use zero_hermes::channels::Channel;

type Log = Arc<Mutex<Vec<(String, String, String)>>>; // (request line, headers, body)

async fn serve(log: Log) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let syncs = Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, syncs) = (log.clone(), syncs.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let (line, head, body) = loop {
                    let n = sock.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let head = text[..i].to_string();
                        let len = head
                            .to_lowercase()
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: ").map(str::to_string))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if text.len() >= i + 4 + len {
                            let line = head.lines().next().unwrap_or("").to_string();
                            break (line, head, text[i + 4..i + 4 + len].to_string());
                        }
                    }
                };
                log.lock().unwrap().push((line.clone(), head, body));
                let payload = if line.contains("/account/whoami") {
                    json!({"user_id": "@bot:hs"})
                } else if line.contains("/sync?") {
                    match syncs.fetch_add(1, Ordering::SeqCst) {
                        0 => {
                            json!({"next_batch": "s1", "rooms": {"join": {"!ok:hs": {"timeline": {"events": [
                            {"type":"m.room.message","sender":"@u:hs","content":{"msgtype":"m.text","body":"backlog"}}]}}}}})
                        }
                        1 => json!({"next_batch": "s2", "rooms": {
                            "join": {"!ok:hs": {"timeline": {"events": [
                                {"type":"m.room.message","sender":"@u:hs","content":{"msgtype":"m.text","body":"fresh"}},
                                {"type":"m.room.message","sender":"@bot:hs","content":{"msgtype":"m.text","body":"echo of me"}}]}}},
                            "invite": {"!new:hs": {"invite_state": {"events": [
                                {"type":"m.room.member","state_key":"@bot:hs","sender":"@boss:hs","content":{"membership":"invite"}}]}}}}}),
                        _ => {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            json!({"next_batch": "s2"})
                        }
                    }
                } else if line.contains("/join/") {
                    json!({"room_id": "!new:hs"})
                } else {
                    json!({"event_id": "$e"})
                };
                let body = payload.to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    port
}

#[tokio::test]
async fn matrix_channel_dispatches_fresh_messages_joins_invites_and_replies() {
    let log: Log = Arc::default();
    let port = serve(log.clone()).await;
    let channel = Arc::new(MatrixChannel::new(MatrixConfig {
        homeserver: format!("http://127.0.0.1:{port}"),
        access_token: "tok".into(),
        allowed_rooms: vec!["!ok:hs".into()],
        allowed_users: vec!["@boss:hs".into()],
        allow_all: false,
        poll_timeout: 1,
    }));
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let runner = channel.clone();
    let task = tokio::spawn(async move { runner.run(tx).await });

    let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("a message within 5s")
        .expect("channel open");
    assert_eq!(
        msg.text, "fresh",
        "the first sync's backlog is not dispatched"
    );
    assert_eq!(msg.chat_id, "!ok:hs");
    assert_eq!(msg.channel, "matrix");
    assert!(
        tokio::time::timeout(Duration::from_millis(300), rx.recv())
            .await
            .is_err(),
        "the bot's own message must not come back"
    );

    channel.send("!ok:hs", "pong").await.unwrap();
    task.abort();

    let log = log.lock().unwrap();
    assert!(
        log.iter().any(
            |(l, h, _)| l.starts_with("POST /_matrix/client/v3/join/%21new%3Ahs")
                && h.to_lowercase().contains("authorization: bearer tok")
        ),
        "trusted invite is joined: {log:?}"
    );
    let put = log
        .iter()
        .find(|(l, _, _)| {
            l.starts_with("PUT /_matrix/client/v3/rooms/%21ok%3Ahs/send/m.room.message/")
        })
        .expect("reply PUT");
    assert!(
        put.2.contains("pong") && put.2.contains("m.text"),
        "{put:?}"
    );
}

#[tokio::test]
async fn matrix_rejected_token_stops_the_channel_with_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut b = [0u8; 2048];
                let _ = s.read(&mut b).await;
                let body = r#"{"errcode":"M_UNKNOWN_TOKEN"}"#;
                let _ = s
                    .write_all(
                        format!(
                            "HTTP/1.1 401 Unauthorized\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            });
        }
    });
    let channel = MatrixChannel::new(MatrixConfig {
        homeserver: format!("http://127.0.0.1:{port}"),
        access_token: "bad".into(),
        allowed_rooms: vec![],
        allowed_users: vec![],
        allow_all: true,
        poll_timeout: 1,
    });
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let err = channel.run(tx).await.unwrap_err().to_string();
    assert!(err.contains("rejected the access token"), "{err}");
}

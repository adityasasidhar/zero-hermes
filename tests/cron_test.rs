//! Integration tests for the cron parser and scheduler.

use std::time::Duration;

use zero_hermes::cron::{CronJob, Scheduler};

#[test]
fn parses_every_minute() {
    let job = CronJob::new("tick", "* * * * *", "do").unwrap();
    let next = job.next().unwrap();
    let diff = (next - chrono::Utc::now()).num_seconds();
    assert!((0..=60).contains(&diff));
}

#[test]
fn parses_every_5_minutes() {
    let job = CronJob::new("x", "*/5 * * * *", "y").unwrap();
    let _ = job.next().unwrap();
}

#[test]
fn rejects_garbage() {
    let r = CronJob::new("bad", "not a cron", "prompt");
    assert!(r.is_err());
}

#[test]
fn describe_minimal() {
    let s = zero_hermes::cron::describe("0 0 * * *", chrono::Utc::now()).unwrap();
    assert!(s.contains('T'));
}

#[tokio::test]
async fn scheduler_emits_event_within_window() {
    let job = CronJob::new("hello", "* * * * *", "prompt").unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let handle = Scheduler::start(vec![job], tx).unwrap();
    let evt = tokio::time::timeout(Duration::from_secs(65), rx.recv()).await;
    handle.stop().await;
    let evt = evt.expect("scheduler fires within 65s").unwrap();
    assert_eq!(evt.job, "hello");
}

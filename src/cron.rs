//! Cron expression parsing and a tiny tokio scheduler.
//!
//! For v1 we use the `cron` crate to parse standard 5-field expressions
//! and compute the next firing time. The async scheduler takes a list of
//! jobs and dispatches them when they fire.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::Result;

/// A single cron job.
#[derive(Debug, Clone)]
pub struct CronJob {
    /// Unique name.
    pub name: String,
    /// Parsed cron schedule.
    pub schedule: Schedule,
    /// Prompt to send to the agent when the job fires.
    pub prompt: String,
}

impl CronJob {
    /// Parse a 5-field cron expression (the user-friendly form).
    ///
    /// The underlying `cron` crate expects 6 fields with a leading
    /// seconds field, so we transparently prepend `0` when the caller
    /// supplies the familiar 5-field form.
    pub fn new(name: impl Into<String>, expr: &str, prompt: impl Into<String>) -> Result<Self> {
        let normalized = normalize_cron(expr);
        let schedule = Schedule::from_str(&normalized)
            .map_err(|e| anyhow::anyhow!("invalid cron expression {expr:?}: {e}"))?;
        Ok(Self {
            name: name.into(),
            schedule,
            prompt: prompt.into(),
        })
    }

    /// Compute the next firing time after `now`.
    pub fn next_after(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(&now).next()
    }

    /// Compute the next firing time relative to the current wall clock.
    pub fn next(&self) -> Option<DateTime<Utc>> {
        self.next_after(Utc::now())
    }
}

/// Convert a 5-field cron expression into the 6-field form expected by
/// the `cron` crate. If the input already has 6 or 7 fields it is returned
/// unchanged.
pub fn normalize_cron(expr: &str) -> String {
    let count = expr.split_whitespace().count();
    match count {
        5 => format!("0 {expr}"),
        6 | 7 => expr.to_string(),
        _ => expr.to_string(),
    }
}

/// Wire format for parsed jobs (useful for `serialize`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobDto {
    pub name: String,
    pub schedule: String,
    pub prompt: String,
}

impl From<&CronJob> for CronJobDto {
    fn from(j: &CronJob) -> Self {
        Self {
            name: j.name.clone(),
            schedule: j.schedule.to_string(),
            prompt: j.prompt.clone(),
        }
    }
}

/// An event emitted when a job fires.
#[derive(Debug, Clone)]
pub struct CronEvent {
    pub job: String,
    pub at: DateTime<Utc>,
}

/// A handle returned by [`Scheduler::start`]. Drop it to stop the loop.
pub struct SchedulerHandle {
    stop: mpsc::Sender<()>,
}

impl SchedulerHandle {
    /// Stop the scheduler.
    pub async fn stop(self) {
        let _ = self.stop.send(()).await;
    }
}

/// Tokio scheduler that fires jobs at their next scheduled time.
pub struct Scheduler;

impl Scheduler {
    /// Start a background loop that emits events on `tx`.
    /// Returns a handle that can be used to stop the loop.
    pub fn start(jobs: Vec<CronJob>, tx: mpsc::Sender<CronEvent>) -> Option<SchedulerHandle> {
        if jobs.is_empty() {
            return None;
        }
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let jobs = Arc::new(jobs);

        tokio::spawn(async move {
            loop {
                let now = Utc::now();
                let mut next_fire: Option<(DateTime<Utc>, String, String)> = None;
                for job in jobs.iter() {
                    if let Some(t) = job.next_after(now) {
                        let replace = match next_fire {
                            Some((when, _, _)) => t < when,
                            None => true,
                        };
                        if replace {
                            next_fire = Some((t, job.name.clone(), job.prompt.clone()));
                        }
                    }
                }
                let Some((when, name, _prompt)) = next_fire else {
                    // No job has a future firing; idle.
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(60)) => {},
                        _ = stop_rx.recv() => break,
                    }
                    continue;
                };

                let dur = (when - now).to_std().unwrap_or(Duration::ZERO);
                tokio::select! {
                    _ = tokio::time::sleep(dur) => {
                        let _ = tx.send(CronEvent { job: name, at: when }).await;
                    }
                    _ = stop_rx.recv() => break,
                }
            }
        });

        Some(SchedulerHandle { stop: stop_tx })
    }
}

/// Parse a cron expression and return its next firing time (string format).
/// Useful for CLI `cron list`.
pub fn describe(expr: &str, now: DateTime<Utc>) -> Result<String> {
    let normalized = normalize_cron(expr);
    let schedule = Schedule::from_str(&normalized)
        .map_err(|e| anyhow::anyhow!("invalid cron expression {expr:?}: {e}"))?;
    let next = schedule
        .after(&now)
        .next()
        .ok_or_else(|| anyhow::anyhow!("no future firing for {expr:?}"))?;
    Ok(next.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_minute() {
        let j = CronJob::new("ping", "* * * * *", "ping").unwrap();
        let next = j.next().unwrap();
        let diff = (next - Utc::now()).num_seconds();
        assert!(
            (0..=60).contains(&diff),
            "next firing within 60s, got {diff}"
        );
    }

    #[test]
    fn parses_specific_time() {
        let j = CronJob::new("hourly", "0 * * * *", "tick").unwrap();
        let _ = j.next().unwrap();
    }

    #[test]
    fn invalid_expr_returns_error() {
        let r = CronJob::new("bad", "not a cron", "x");
        assert!(r.is_err());
    }

    #[test]
    fn describe_pretty() {
        let s = describe("0 0 * * *", Utc::now()).unwrap();
        assert!(s.contains('T'));
    }

    #[tokio::test]
    async fn scheduler_emits_event() {
        let job = CronJob::new("tick", "* * * * *", "do").unwrap();
        let (tx, mut rx) = mpsc::channel(1);
        let handle = Scheduler::start(vec![job], tx).unwrap();
        let evt = tokio::time::timeout(Duration::from_secs(65), rx.recv()).await;
        handle.stop().await;
        let evt = evt.expect("scheduler fires within 65s").unwrap();
        assert_eq!(evt.job, "tick");
    }
}

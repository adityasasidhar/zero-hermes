//! Cron expression parsing and a tiny tokio scheduler.
//!
//! For v1 we use the `cron` crate to parse standard 5-field expressions
//! and compute the next firing time. The async scheduler takes a list of
//! jobs and dispatches them when they fire.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::config::CronTimezone;
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

    /// Compute the next firing time after `now`, interpreting the schedule
    /// as UTC.
    pub fn next_after(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.next_after_in(now, CronTimezone::Utc)
    }

    /// Compute the next firing time after `now` against a chosen clock.
    ///
    /// `0 9 * * *` means 9am UTC by default; with [`CronTimezone::Local`]
    /// it means 9am wherever the host is, which is almost always what
    /// someone writing a schedule by hand expects.
    pub fn next_after_in(&self, now: DateTime<Utc>, tz: CronTimezone) -> Option<DateTime<Utc>> {
        match tz {
            CronTimezone::Utc => self.schedule.after(&now).next(),
            CronTimezone::Local => self
                .schedule
                .after(&now.with_timezone(&Local))
                .next()
                .map(|t| t.with_timezone(&Utc)),
        }
    }

    /// Compute the next firing time relative to the current wall clock.
    pub fn next(&self) -> Option<DateTime<Utc>> {
        self.next_after(Utc::now())
    }
}

/// Convert a 5-field cron expression into the 6-field form expected by
/// the `cron` crate. If the input already has 6 or 7 fields it is returned
/// unchanged. Anything else is returned unchanged and will be rejected by
/// the parser.
pub fn normalize_cron(expr: &str) -> String {
    let count = expr.split_whitespace().count();
    if count == 5 {
        format!("0 {expr}")
    } else {
        expr.to_string()
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
    /// Name of the job that fired.
    pub job: String,
    /// When the job fired (scheduled wall-clock time).
    pub at: DateTime<Utc>,
    /// The prompt configured for this job. Carried along so the consumer
    /// can dispatch it without re-deriving from the job name.
    pub prompt: String,
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
        Self::start_in(jobs, CronTimezone::Utc, tx)
    }

    /// Start the scheduler, interpreting schedules against `tz`.
    pub fn start_in(
        jobs: Vec<CronJob>,
        tz: CronTimezone,
        tx: mpsc::Sender<CronEvent>,
    ) -> Option<SchedulerHandle> {
        if jobs.is_empty() {
            return None;
        }
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        let jobs = Arc::new(jobs);

        tokio::spawn(async move {
            loop {
                let now = Utc::now();
                // Find the earliest firing time, then collect *every* job
                // due at that instant. Firing only the first one meant two
                // jobs sharing a schedule silently became one: after the
                // winner fired, the loop recomputed from a `now` already
                // past the shared instant, so the loser skipped straight
                // to its next occurrence.
                let mut next_fire: Option<DateTime<Utc>> = None;
                for job in jobs.iter() {
                    if let Some(t) = job.next_after_in(now, tz) {
                        if next_fire.is_none_or(|when| t < when) {
                            next_fire = Some(t);
                        }
                    }
                }
                let Some(when) = next_fire else {
                    // No job has a future firing; idle.
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(60)) => {},
                        _ = stop_rx.recv() => break,
                    }
                    continue;
                };
                let due: Vec<&CronJob> = jobs
                    .iter()
                    .filter(|j| j.next_after_in(now, tz) == Some(when))
                    .collect();

                let dur = (when - now).to_std().unwrap_or(Duration::ZERO);
                let events: Vec<CronEvent> = due
                    .iter()
                    .map(|job| CronEvent {
                        job: job.name.clone(),
                        at: when,
                        prompt: job.prompt.clone(),
                    })
                    .collect();
                tokio::select! {
                    _ = tokio::time::sleep(dur) => {
                        for event in events {
                            let name = event.job.clone();
                            if tx.send(event).await.is_err() {
                                // Receiver gone; nothing to do but keep the
                                // scheduler idle until stop or the next tick.
                                tracing::info!(job = %name, "cron receiver closed; scheduler idling");
                                break;
                            }
                        }
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

    // `start_paused` auto-advances tokio's clock whenever every task is
    // idle, so the scheduler's `sleep` to the next minute boundary resolves
    // immediately. This test used to burn up to 60s of real wall time and
    // dominated the whole suite.
    #[tokio::test(start_paused = true)]
    async fn scheduler_emits_event() {
        let job = CronJob::new("tick", "* * * * *", "do").unwrap();
        let (tx, mut rx) = mpsc::channel(1);
        let handle = Scheduler::start(vec![job], tx).unwrap();
        let evt = tokio::time::timeout(Duration::from_secs(65), rx.recv()).await;
        let evt = evt.expect("scheduler fires within 65s").unwrap();
        handle.stop().await;
        assert_eq!(evt.job, "tick");
        assert_eq!(evt.prompt, "do");
    }

    #[tokio::test(start_paused = true)]
    async fn scheduler_fires_every_job_due_at_the_same_instant() {
        // Three jobs on an identical schedule must all fire. Picking only
        // the earliest job meant the other two were skipped to their next
        // occurrence — one tick per minute instead of three.
        let jobs = vec![
            CronJob::new("a", "* * * * *", "pa").unwrap(),
            CronJob::new("b", "* * * * *", "pb").unwrap(),
            CronJob::new("c", "* * * * *", "pc").unwrap(),
        ];
        let (tx, mut rx) = mpsc::channel(8);
        let handle = Scheduler::start(jobs, tx).unwrap();

        let mut fired = Vec::new();
        for _ in 0..3 {
            let evt = tokio::time::timeout(Duration::from_secs(65), rx.recv())
                .await
                .expect("all three jobs fire on the same tick")
                .unwrap();
            fired.push((evt.job, evt.at));
        }
        handle.stop().await;

        let names: std::collections::BTreeSet<&str> =
            fired.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"].into_iter().collect());
        assert!(
            fired.iter().all(|(_, at)| *at == fired[0].1),
            "all three should report the same firing instant: {fired:?}"
        );
    }

    #[test]
    fn local_and_utc_schedules_agree_on_interval_expressions() {
        // "every 5 minutes" is timezone-independent, so both clocks must
        // land on the same instant — this checks the Local conversion
        // round-trips rather than shifting by the UTC offset.
        let job = CronJob::new("j", "*/5 * * * *", "p").unwrap();
        let now = Utc::now();
        assert_eq!(
            job.next_after_in(now, CronTimezone::Utc),
            job.next_after_in(now, CronTimezone::Local)
        );
    }

    #[test]
    fn local_daily_schedule_respects_the_host_offset() {
        use chrono::Timelike;
        let job = CronJob::new("daily", "0 9 * * *", "p").unwrap();
        let now = Utc::now();
        let local_next = job
            .next_after_in(now, CronTimezone::Local)
            .expect("has a next firing");
        assert_eq!(
            local_next.with_timezone(&Local).hour(),
            9,
            "a local schedule should fire at 9am local time"
        );
        let utc_next = job
            .next_after_in(now, CronTimezone::Utc)
            .expect("has a next firing");
        assert_eq!(utc_next.hour(), 9, "a UTC schedule should fire at 9am UTC");
    }
}

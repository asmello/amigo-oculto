//! Hourly housekeeping: expired verifications, sessions and login links, delivered mail older
//! than a month, rate-limit events, and whole games once the event is far enough in the past.

use std::time::Duration;

use anyhow::Result;
use chrono::Duration as ChronoDuration;
use tokio_util::sync::CancellationToken;

use crate::db::{self, SqlitePool};
use crate::fmt;
use crate::repo;

const INTERVAL: Duration = Duration::from_secs(3600);

pub async fn run(db: SqlitePool, retention_days: u32, cancel: CancellationToken) {
    // First pass shortly after boot so a restarting server still cleans up.
    let mut delay = Duration::from_secs(30);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(delay) => {}
        }
        match run_once(&db, retention_days).await {
            Ok(report) => tracing::info!(%report, "cleanup done"),
            Err(e) => tracing::error!(error = %e, "cleanup failed"),
        }
        delay = INTERVAL;
    }
    tracing::debug!("cleanup task stopped");
}

#[derive(Debug, Default)]
pub struct Report {
    pub games: u64,
    pub verifications: u64,
    pub auth: u64,
    pub emails: u64,
    pub rate_events: u64,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "games={} verifications={} sessions_tokens={} emails={} rate_events={}",
            self.games, self.verifications, self.auth, self.emails, self.rate_events
        )
    }
}

pub async fn run_once(db: &SqlitePool, retention_days: u32) -> Result<Report> {
    let now = db::now();
    let cutoff = fmt::today() - ChronoDuration::days(i64::from(retention_days));
    let mut conn = db.acquire().await?;
    let report = Report {
        games: repo::games::delete_with_event_before(&mut conn, cutoff).await?,
        verifications: repo::verifications::delete_expired_before(&mut conn, now - 86_400).await?,
        auth: repo::auth::delete_expired(&mut conn, now).await?,
        emails: repo::outbox::delete_finished_before(&mut conn, now - 30 * 86_400).await?,
        rate_events: repo::rate::delete_before(&mut conn, now - 2 * 86_400).await?,
    };
    Ok(report)
}

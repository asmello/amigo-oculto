//! Background delivery of queued emails with retries.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::db::{self, SqlitePool};
use crate::mail::{Mailer, SendError};
use crate::repo::outbox;

const BATCH: i64 = 25;
const POLL_INTERVAL: Duration = Duration::from_secs(10);
/// Delay before attempt N+1 after N failures: 30 s, 2 min, 10 min, 1 h, 6 h; then give up.
const BACKOFF_SECS: [i64; 5] = [30, 120, 600, 3600, 21_600];

/// Runs until `cancel` fires. Wakes on `notify` (new mail queued) or every `POLL_INTERVAL`.
pub async fn run(db: SqlitePool, mailer: Mailer, notify: Arc<Notify>, cancel: CancellationToken) {
    loop {
        match process_due(&db, &mailer).await {
            Ok(0) => {}
            Ok(n) => tracing::debug!(sent_or_retried = n, "outbox batch processed"),
            Err(e) => tracing::error!(error = %e, "outbox worker iteration failed"),
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = notify.notified() => {}
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
    tracing::debug!("outbox worker stopped");
}

/// Sends every due email once. Returns how many were attempted. Also used by tests to flush
/// the queue synchronously.
pub async fn process_due(db: &SqlitePool, mailer: &Mailer) -> Result<usize> {
    let now = db::now();
    let due = {
        let mut conn = db.acquire().await?;
        outbox::fetch_due(&mut conn, now, BATCH).await?
    };
    let count = due.len();
    for email in due {
        let result = mailer.send(&email).await;
        let mut conn = db.acquire().await?;
        match result {
            Ok(()) => {
                tracing::info!(kind = ?email.kind, to = %email.to_email, "email sent");
                outbox::mark_sent(&mut conn, email.id, db::now()).await?;
            }
            Err(SendError::Permanent(msg)) => {
                tracing::warn!(kind = ?email.kind, to = %email.to_email, error = %msg, "email rejected");
                outbox::mark_failed(&mut conn, email.id, &msg).await?;
            }
            Err(SendError::Transient(msg)) => {
                let attempt = usize::try_from(email.attempts).unwrap_or(usize::MAX);
                match BACKOFF_SECS.get(attempt) {
                    Some(delay) => {
                        tracing::warn!(kind = ?email.kind, to = %email.to_email, error = %msg, retry_in_secs = delay, "email deferred");
                        outbox::mark_retry(&mut conn, email.id, db::now() + delay, &msg).await?;
                    }
                    None => {
                        tracing::error!(kind = ?email.kind, to = %email.to_email, error = %msg, "email gave up");
                        outbox::mark_failed(&mut conn, email.id, &msg).await?;
                    }
                }
            }
        }
    }
    Ok(count)
}

//! Background task management for the server.
//!
//! Cleanup tasks are staggered to avoid concurrent writes to SQLite, which can cause
//! "database is locked" errors. Each task runs once at startup (to handle frequently
//! restarting servers), then continues on a regular interval.

use crate::db::Database;
use anyhow::Result;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::JoinMap;

const CLEANUP_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour

/// Stagger between cleanup tasks to avoid concurrent SQLite writes.
const CLEANUP_STAGGER: Duration = Duration::from_secs(5);

pub struct Server {
    /// JoinMap allows us to associate each task with a name, which is returned
    /// when the task completes so we can log which task stopped.
    tasks: JoinMap<&'static str, ()>,
}

impl Server {
    pub fn new(db: &Database, cancel: CancellationToken) -> Result<Self> {
        let mut tasks = JoinMap::new();
        let now = Instant::now();
        tasks.spawn(
            "cleanup_verifications",
            Self::cleanup_verifications_task(db.clone(), cancel.clone(), now),
        );
        tasks.spawn(
            "cleanup_games",
            Self::cleanup_games_task(db.clone(), cancel.clone(), now + CLEANUP_STAGGER),
        );
        tasks.spawn(
            "cleanup_admin_sessions",
            Self::cleanup_admin_sessions_task(db.clone(), cancel, now + CLEANUP_STAGGER * 2),
        );
        Ok(Self { tasks })
    }

    /// Waits for all background tasks to complete.
    pub async fn shutdown(mut self) {
        while let Some((name, result)) = self.tasks.join_next().await {
            match result {
                Ok(()) => tracing::debug!(task = name, "task stopped"),
                Err(err) if err.is_cancelled() => {
                    tracing::debug!(task = name, "task aborted");
                }
                Err(err) => std::panic::resume_unwind(err.into_panic()),
            }
        }
        tracing::info!("all background tasks have stopped");
    }

    async fn cleanup_verifications_task(db: Database, cancel: CancellationToken, start: Instant) {
        // Wait for staggered start time
        tokio::select! {
            _ = tokio::time::sleep_until(start) => {}
            _ = cancel.cancelled() => {
                tracing::trace!("cleanup verifications task received shutdown signal");
                return;
            }
        }

        // Run cleanup once at startup, then on interval
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            match db.cleanup_expired_verifications().await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {} expired verification(s)", count);
                }
                Ok(_) => {
                    tracing::debug!("no expired verifications to clean up");
                }
                Err(e) => {
                    tracing::error!("failed to cleanup expired verifications: {}", e);
                }
            }

            tokio::select! {
                _ = interval.tick() => {}
                _ = cancel.cancelled() => {
                    tracing::trace!("cleanup verifications task received shutdown signal");
                    break;
                }
            }
        }
    }

    async fn cleanup_games_task(db: Database, cancel: CancellationToken, start: Instant) {
        // Wait for staggered start time
        tokio::select! {
            _ = tokio::time::sleep_until(start) => {}
            _ = cancel.cancelled() => {
                tracing::trace!("cleanup games task received shutdown signal");
                return;
            }
        }

        // Run cleanup once at startup, then on interval
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            match db.cleanup_old_games().await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {} old game(s)", count);
                }
                Ok(_) => {
                    tracing::debug!("no old games to clean up");
                }
                Err(e) => {
                    tracing::error!("failed to cleanup old games: {}", e);
                }
            }

            tokio::select! {
                _ = interval.tick() => {}
                _ = cancel.cancelled() => {
                    tracing::trace!("cleanup games task received shutdown signal");
                    break;
                }
            }
        }
    }

    async fn cleanup_admin_sessions_task(db: Database, cancel: CancellationToken, start: Instant) {
        // Wait for staggered start time
        tokio::select! {
            _ = tokio::time::sleep_until(start) => {}
            _ = cancel.cancelled() => {
                tracing::trace!("cleanup admin sessions task received shutdown signal");
                return;
            }
        }

        // Run cleanup once at startup, then on interval
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            match db.cleanup_expired_admin_sessions().await {
                Ok(count) if count > 0 => {
                    tracing::info!("cleaned up {} expired admin session(s)", count);
                }
                Ok(_) => {
                    tracing::debug!("no expired admin sessions to clean up");
                }
                Err(e) => {
                    tracing::error!("failed to cleanup expired admin sessions: {}", e);
                }
            }

            tokio::select! {
                _ = interval.tick() => {}
                _ = cancel.cancelled() => {
                    tracing::trace!("cleanup admin sessions task received shutdown signal");
                    break;
                }
            }
        }
    }
}

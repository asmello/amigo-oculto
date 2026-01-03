use crate::db::Database;
use anyhow::Result;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct Server {
    tasks: JoinSet<()>,
}

impl Server {
    pub fn new(db: &Database, cancel: CancellationToken) -> Result<Self> {
        let mut tasks = JoinSet::new();
        tasks.spawn(Self::cleanup_verifications_task(db.clone(), cancel.clone()));
        tasks.spawn(Self::cleanup_games_task(db.clone(), cancel.clone()));
        tasks.spawn(Self::cleanup_admin_sessions_task(db.clone(), cancel));
        Ok(Self { tasks })
    }

    /// Waits for all background tasks to complete.
    pub async fn shutdown(mut self) {
        while let Some(next) = self.tasks.join_next().await {
            if let Err(err) = next {
                if err.is_cancelled() {
                    tracing::debug!("task aborted");
                    continue;
                }
                std::panic::resume_unwind(err.into_panic())
            }
        }
        tracing::info!("all background tasks have stopped");
    }

    async fn cleanup_verifications_task(db: Database, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = interval.tick() => {
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
                }
                _ = cancel.cancelled() => {
                    tracing::debug!("cleanup verifications task received shutdown signal");
                    break;
                }
            }
        }
    }

    async fn cleanup_games_task(db: Database, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = interval.tick() => {
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
                }
                _ = cancel.cancelled() => {
                    tracing::debug!("cleanup games task received shutdown signal");
                    break;
                }
            }
        }
    }

    async fn cleanup_admin_sessions_task(db: Database, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = interval.tick() => {
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
                }
                _ = cancel.cancelled() => {
                    tracing::debug!("cleanup admin sessions task received shutdown signal");
                    break;
                }
            }
        }
    }
}

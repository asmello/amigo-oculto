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
        tasks.spawn(Self::cleanup_task(db.clone(), cancel));
        Ok(Self { tasks })
    }

    /// Waits for all background tasks to complete.
    pub async fn shutdown(mut self) {
        while self.tasks.join_next().await.is_some() {}
        tracing::info!("All background tasks have stopped");
    }

    async fn cleanup_task(db: Database, cancel: CancellationToken) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match db.cleanup_expired_verifications().await {
                        Ok(count) if count > 0 => {
                            tracing::info!("Cleaned up {} expired verification(s)", count);
                        }
                        Ok(_) => {
                            tracing::debug!("No expired verifications to clean up");
                        }
                        Err(e) => {
                            tracing::error!("Failed to cleanup expired verifications: {}", e);
                        }
                    }
                }
                _ = cancel.cancelled() => {
                    tracing::debug!("Cleanup task received shutdown signal");
                    break;
                }
            }
        }
    }
}

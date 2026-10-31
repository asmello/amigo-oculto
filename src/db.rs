//! Database connection pool and migrations.

use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};
pub use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

/// Current time as unix seconds — the representation used for every timestamp column.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Opens (creating if needed) the database at `url` and applies pending migrations.
pub async fn connect(url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)
        .with_context(|| format!("parsing DATABASE_URL {url:?}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        // WAL is what Litestream replicates; NORMAL sync is durable enough under WAL.
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .context("opening database")?;
    migrate(&pool).await?;
    Ok(pool)
}

/// In-memory database for tests. A single connection keeps the data alive for the pool's life.
pub async fn connect_in_memory() -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("running migrations")
}

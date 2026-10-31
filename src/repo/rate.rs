//! Persistent sliding-window rate limits keyed by arbitrary strings
//! (e.g. `verify:<email>`, `resend:<participant>`).

use anyhow::Result;

use super::Exec;

/// Returns true (and records the event) if fewer than `limit` events for `key` happened in the
/// last `window_secs`; false if the limit is exhausted.
pub async fn allow(
    ex: Exec<'_>,
    key: &str,
    limit: i64,
    window_secs: i64,
    now: i64,
) -> Result<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rate_events WHERE key = ? AND at > ?")
        .bind(key)
        .bind(now - window_secs)
        .fetch_one(&mut *ex)
        .await?;
    if n >= limit {
        return Ok(false);
    }
    sqlx::query("INSERT INTO rate_events (key, at) VALUES (?, ?)")
        .bind(key)
        .bind(now)
        .execute(&mut *ex)
        .await?;
    Ok(true)
}

pub async fn delete_before(ex: Exec<'_>, before: i64) -> Result<u64> {
    let r = sqlx::query("DELETE FROM rate_events WHERE at < ?")
        .bind(before)
        .execute(ex)
        .await?;
    Ok(r.rows_affected())
}

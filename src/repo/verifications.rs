use anyhow::Result;

use super::Exec;
use crate::email_addr::EmailAddress;
use crate::ids::{SecretHash, VerificationId};
use crate::models::Verification;

const COLUMNS: &str = "id, email, code_hash, name, event_date, budget_cents, message, organizer_name, \
                       attempts, expires_at, consumed_at, created_at";

pub async fn insert(ex: Exec<'_>, v: &Verification) -> Result<()> {
    sqlx::query(
        "INSERT INTO verifications (id, email, code_hash, name, event_date, budget_cents, message, \
         organizer_name, attempts, expires_at, consumed_at, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(v.id)
    .bind(&v.email)
    .bind(&v.code_hash)
    .bind(&v.name)
    .bind(v.event_date)
    .bind(v.budget_cents)
    .bind(&v.message)
    .bind(&v.organizer_name)
    .bind(v.attempts)
    .bind(v.expires_at)
    .bind(v.consumed_at)
    .bind(v.created_at)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn get(ex: Exec<'_>, id: VerificationId) -> Result<Option<Verification>> {
    let v = sqlx::query_as::<_, Verification>(&format!(
        "SELECT {COLUMNS} FROM verifications WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(ex)
    .await?;
    Ok(v)
}

/// Increments the attempt counter and returns the new value.
pub async fn record_attempt(ex: Exec<'_>, id: VerificationId) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(
        "UPDATE verifications SET attempts = attempts + 1 WHERE id = ? RETURNING attempts",
    )
    .bind(id)
    .fetch_one(ex)
    .await?;
    Ok(n)
}

/// Claims the verification. Returns false when it had already been consumed, which is how a
/// duplicate submit of the same code (double tap, retried request) loses the race: only the
/// request that flips `consumed_at` may create the game.
pub async fn consume(ex: Exec<'_>, id: VerificationId, now: i64) -> Result<bool> {
    let r = sqlx::query(
        "UPDATE verifications SET consumed_at = ? WHERE id = ? AND consumed_at IS NULL",
    )
    .bind(now)
    .bind(id)
    .execute(ex)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn replace_code(
    ex: Exec<'_>,
    id: VerificationId,
    code_hash: &SecretHash,
    expires_at: i64,
) -> Result<()> {
    sqlx::query(
        "UPDATE verifications SET code_hash = ?, expires_at = ?, attempts = 0 WHERE id = ?",
    )
    .bind(code_hash)
    .bind(expires_at)
    .bind(id)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn count_recent(ex: Exec<'_>, email: &EmailAddress, since: i64) -> Result<i64> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM verifications WHERE email = ? AND created_at >= ?")
            .bind(email)
            .bind(since)
            .fetch_one(ex)
            .await?;
    Ok(n)
}

pub async fn delete_expired_before(ex: Exec<'_>, before: i64) -> Result<u64> {
    let r = sqlx::query("DELETE FROM verifications WHERE expires_at < ?")
        .bind(before)
        .execute(ex)
        .await?;
    Ok(r.rows_affected())
}

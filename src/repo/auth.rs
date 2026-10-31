use anyhow::Result;

use super::Exec;
use crate::email_addr::EmailAddress;
use crate::ids::SecretHash;
use crate::models::{LoginToken, Session, SessionKind};

pub async fn insert_login_token(ex: Exec<'_>, t: &LoginToken) -> Result<()> {
    sqlx::query(
        "INSERT INTO login_tokens (token_hash, email, kind, expires_at, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&t.token_hash)
    .bind(&t.email)
    .bind(t.kind)
    .bind(t.expires_at)
    .bind(t.created_at)
    .execute(ex)
    .await?;
    Ok(())
}

/// Returns the token if it exists and has not expired.
pub async fn get_login_token(
    ex: Exec<'_>,
    hash: &SecretHash,
    now: i64,
) -> Result<Option<LoginToken>> {
    let t = sqlx::query_as::<_, LoginToken>(
        "SELECT token_hash, email, kind, expires_at, created_at FROM login_tokens \
         WHERE token_hash = ? AND expires_at > ?",
    )
    .bind(hash)
    .bind(now)
    .fetch_optional(ex)
    .await?;
    Ok(t)
}

pub async fn delete_login_tokens_for(
    ex: Exec<'_>,
    email: &EmailAddress,
    kind: SessionKind,
) -> Result<()> {
    sqlx::query("DELETE FROM login_tokens WHERE email = ? AND kind = ?")
        .bind(email)
        .bind(kind)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn insert_session(ex: Exec<'_>, s: &Session) -> Result<()> {
    sqlx::query("INSERT INTO sessions (id_hash, email, kind, created_at, expires_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&s.id_hash)
        .bind(&s.email)
        .bind(s.kind)
        .bind(s.created_at)
        .bind(s.expires_at)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn get_session(ex: Exec<'_>, hash: &SecretHash, now: i64) -> Result<Option<Session>> {
    let s = sqlx::query_as::<_, Session>(
        "SELECT id_hash, email, kind, created_at, expires_at FROM sessions WHERE id_hash = ? AND expires_at > ?",
    )
    .bind(hash)
    .bind(now)
    .fetch_optional(ex)
    .await?;
    Ok(s)
}

pub async fn delete_session(ex: Exec<'_>, hash: &SecretHash) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE id_hash = ?")
        .bind(hash)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn delete_expired(ex: Exec<'_>, now: i64) -> Result<u64> {
    let a = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(&mut *ex)
        .await?;
    let b = sqlx::query("DELETE FROM login_tokens WHERE expires_at <= ?")
        .bind(now)
        .execute(&mut *ex)
        .await?;
    Ok(a.rows_affected() + b.rows_affected())
}

use anyhow::Result;

use super::Exec;
use crate::email_addr::EmailAddress;
use crate::ids::{GameId, ParticipantId, SecretHash};
use crate::models::Participant;

const COLUMNS: &str = "id, game_id, name, email, wishlist, receiver_id, reveal_token_hash, \
                       revealed_at, position, created_at";

pub async fn insert(ex: Exec<'_>, p: &Participant) -> Result<()> {
    sqlx::query(
        "INSERT INTO participants (id, game_id, name, email, wishlist, receiver_id, \
         reveal_token_hash, revealed_at, position, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(p.id)
    .bind(p.game_id)
    .bind(&p.name)
    .bind(&p.email)
    .bind(&p.wishlist)
    .bind(p.receiver_id)
    .bind(&p.reveal_token_hash)
    .bind(p.revealed_at)
    .bind(p.position)
    .bind(p.created_at)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn list(ex: Exec<'_>, game_id: GameId) -> Result<Vec<Participant>> {
    let rows = sqlx::query_as::<_, Participant>(&format!(
        "SELECT {COLUMNS} FROM participants WHERE game_id = ? ORDER BY position, created_at"
    ))
    .bind(game_id)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub async fn get(ex: Exec<'_>, id: ParticipantId) -> Result<Option<Participant>> {
    let p = sqlx::query_as::<_, Participant>(&format!(
        "SELECT {COLUMNS} FROM participants WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(ex)
    .await?;
    Ok(p)
}

pub async fn get_by_token_hash(ex: Exec<'_>, hash: &SecretHash) -> Result<Option<Participant>> {
    let p = sqlx::query_as::<_, Participant>(&format!(
        "SELECT {COLUMNS} FROM participants WHERE reveal_token_hash = ?"
    ))
    .bind(hash)
    .fetch_optional(ex)
    .await?;
    Ok(p)
}

pub async fn list_by_email(ex: Exec<'_>, email: &EmailAddress) -> Result<Vec<Participant>> {
    let rows = sqlx::query_as::<_, Participant>(&format!(
        "SELECT {COLUMNS} FROM participants WHERE email = ? ORDER BY created_at DESC"
    ))
    .bind(email)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub async fn count(ex: Exec<'_>, game_id: GameId) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM participants WHERE game_id = ?")
        .bind(game_id)
        .fetch_one(ex)
        .await?;
    Ok(n)
}

pub async fn next_position(ex: Exec<'_>, game_id: GameId) -> Result<i64> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COALESCE(MAX(position), 0) + 1 FROM participants WHERE game_id = ?")
            .bind(game_id)
            .fetch_one(ex)
            .await?;
    Ok(n)
}

pub async fn email_exists(ex: Exec<'_>, game_id: GameId, email: &EmailAddress) -> Result<bool> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM participants WHERE game_id = ? AND email = ?")
            .bind(game_id)
            .bind(email)
            .fetch_one(ex)
            .await?;
    Ok(n > 0)
}

pub async fn update_name_email(
    ex: Exec<'_>,
    id: ParticipantId,
    name: &str,
    email: &EmailAddress,
) -> Result<()> {
    sqlx::query("UPDATE participants SET name = ?, email = ? WHERE id = ?")
        .bind(name)
        .bind(email)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn set_token_hash(ex: Exec<'_>, id: ParticipantId, hash: &SecretHash) -> Result<()> {
    sqlx::query("UPDATE participants SET reveal_token_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn set_wishlist(ex: Exec<'_>, id: ParticipantId, wishlist: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE participants SET wishlist = ? WHERE id = ?")
        .bind(wishlist)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

/// Records the first reveal. Later reveals keep the original timestamp.
pub async fn mark_revealed(ex: Exec<'_>, id: ParticipantId, now: i64) -> Result<()> {
    sqlx::query("UPDATE participants SET revealed_at = COALESCE(revealed_at, ?) WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn remove(ex: Exec<'_>, id: ParticipantId) -> Result<bool> {
    let r = sqlx::query("DELETE FROM participants WHERE id = ?")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Applies a draw: sets who each participant gives to, rotates every reveal token and clears
/// reveal timestamps. Must run inside the caller's transaction.
pub async fn apply_draw(
    ex: Exec<'_>,
    assignments: &[(ParticipantId, ParticipantId, SecretHash)],
) -> Result<()> {
    for (giver, receiver, token_hash) in assignments {
        sqlx::query(
            "UPDATE participants SET receiver_id = ?, reveal_token_hash = ?, revealed_at = NULL \
             WHERE id = ?",
        )
        .bind(receiver)
        .bind(token_hash)
        .bind(giver)
        .execute(&mut *ex)
        .await?;
    }
    Ok(())
}

/// Undoes a draw without deleting anyone (used when a removal leaves too few people).
pub async fn clear_draw(ex: Exec<'_>, game_id: GameId) -> Result<()> {
    sqlx::query("UPDATE participants SET receiver_id = NULL, revealed_at = NULL WHERE game_id = ?")
        .bind(game_id)
        .execute(ex)
        .await?;
    Ok(())
}

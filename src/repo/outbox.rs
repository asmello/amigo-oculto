use std::collections::HashMap;

use anyhow::Result;

use super::Exec;
use crate::email_addr::EmailAddress;
use crate::ids::{GameId, OutboxId, ParticipantId};
use crate::models::{EmailKind, OutboxEmail, OutboxStatus};

const COLUMNS: &str = "id, game_id, participant_id, kind, draw_round, to_email, to_name, reply_to, \
                       subject, text_body, html_body, status, attempts, next_attempt_at, last_error, \
                       created_at, sent_at";

/// An email to queue. Rendered bodies are stored so the worker needs no template context.
#[derive(Debug, Clone)]
pub struct NewEmail {
    pub game_id: Option<GameId>,
    pub participant_id: Option<ParticipantId>,
    pub kind: EmailKind,
    pub draw_round: Option<i64>,
    pub to_email: EmailAddress,
    pub to_name: Option<String>,
    pub reply_to: Option<EmailAddress>,
    pub subject: String,
    pub text_body: String,
    pub html_body: String,
}

pub async fn enqueue(ex: Exec<'_>, e: &NewEmail, now: i64) -> Result<OutboxId> {
    let id = OutboxId::new();
    sqlx::query(
        "INSERT INTO outbox (id, game_id, participant_id, kind, draw_round, to_email, to_name, reply_to, \
         subject, text_body, html_body, status, attempts, next_attempt_at, last_error, created_at, sent_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, NULL, ?, NULL)",
    )
    .bind(id)
    .bind(e.game_id)
    .bind(e.participant_id)
    .bind(e.kind)
    .bind(e.draw_round)
    .bind(&e.to_email)
    .bind(&e.to_name)
    .bind(&e.reply_to)
    .bind(&e.subject)
    .bind(&e.text_body)
    .bind(&e.html_body)
    .bind(OutboxStatus::Queued)
    .bind(now)
    .bind(now)
    .execute(ex)
    .await?;
    Ok(id)
}

pub async fn fetch_due(ex: Exec<'_>, now: i64, limit: i64) -> Result<Vec<OutboxEmail>> {
    let rows = sqlx::query_as::<_, OutboxEmail>(&format!(
        "SELECT {COLUMNS} FROM outbox WHERE status = ? AND next_attempt_at <= ? \
         ORDER BY next_attempt_at, created_at LIMIT ?"
    ))
    .bind(OutboxStatus::Queued)
    .bind(now)
    .bind(limit)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub async fn mark_sent(ex: Exec<'_>, id: OutboxId, now: i64) -> Result<()> {
    sqlx::query("UPDATE outbox SET status = ?, attempts = attempts + 1, sent_at = ?, last_error = NULL WHERE id = ?")
        .bind(OutboxStatus::Sent)
        .bind(now)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn mark_retry(
    ex: Exec<'_>,
    id: OutboxId,
    next_attempt_at: i64,
    error: &str,
) -> Result<()> {
    sqlx::query("UPDATE outbox SET attempts = attempts + 1, next_attempt_at = ?, last_error = ? WHERE id = ?")
        .bind(next_attempt_at)
        .bind(error)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn mark_failed(ex: Exec<'_>, id: OutboxId, error: &str) -> Result<()> {
    sqlx::query(
        "UPDATE outbox SET status = ?, attempts = attempts + 1, last_error = ? WHERE id = ?",
    )
    .bind(OutboxStatus::Failed)
    .bind(error)
    .bind(id)
    .execute(ex)
    .await?;
    Ok(())
}

/// Delivery status of the newest invitation per participant for the given draw round.
pub async fn latest_invites(
    ex: Exec<'_>,
    game_id: GameId,
    draw_round: i64,
) -> Result<HashMap<ParticipantId, (OutboxStatus, Option<String>)>> {
    let rows: Vec<(ParticipantId, OutboxStatus, Option<String>)> = sqlx::query_as(
        "SELECT participant_id, status, last_error FROM outbox \
         WHERE game_id = ? AND kind = ? AND draw_round = ? AND participant_id IS NOT NULL \
         ORDER BY created_at ASC",
    )
    .bind(game_id)
    .bind(EmailKind::Invite)
    .bind(draw_round)
    .fetch_all(ex)
    .await?;
    // Ascending order + insert means the newest row wins.
    Ok(rows.into_iter().map(|(p, s, e)| (p, (s, e))).collect())
}

pub async fn count_queued(ex: Exec<'_>) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM outbox WHERE status = ?")
        .bind(OutboxStatus::Queued)
        .fetch_one(ex)
        .await?;
    Ok(n)
}

pub async fn delete_finished_before(ex: Exec<'_>, before: i64) -> Result<u64> {
    let r = sqlx::query("DELETE FROM outbox WHERE status != ? AND created_at < ?")
        .bind(OutboxStatus::Queued)
        .bind(before)
        .execute(ex)
        .await?;
    Ok(r.rows_affected())
}

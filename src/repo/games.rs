use anyhow::Result;
use chrono::NaiveDate;

use super::Exec;
use crate::email_addr::EmailAddress;
use crate::ids::GameId;
use crate::models::{Game, GameStatus};

const COLUMNS: &str = "id, name, event_date, budget_cents, message, organizer_name, organizer_email, \
                       status, draw_round, drawn_at, created_at, updated_at";

pub async fn insert(ex: Exec<'_>, g: &Game) -> Result<()> {
    sqlx::query(
        "INSERT INTO games (id, name, event_date, budget_cents, message, organizer_name, \
         organizer_email, status, draw_round, drawn_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(g.id)
    .bind(&g.name)
    .bind(g.event_date)
    .bind(g.budget_cents)
    .bind(&g.message)
    .bind(&g.organizer_name)
    .bind(&g.organizer_email)
    .bind(g.status)
    .bind(g.draw_round)
    .bind(g.drawn_at)
    .bind(g.created_at)
    .bind(g.updated_at)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn get(ex: Exec<'_>, id: GameId) -> Result<Option<Game>> {
    let g = sqlx::query_as::<_, Game>(&format!("SELECT {COLUMNS} FROM games WHERE id = ?"))
        .bind(id)
        .fetch_optional(ex)
        .await?;
    Ok(g)
}

pub async fn list_by_organizer(ex: Exec<'_>, email: &EmailAddress) -> Result<Vec<Game>> {
    let rows = sqlx::query_as::<_, Game>(&format!(
        "SELECT {COLUMNS} FROM games WHERE organizer_email = ? ORDER BY event_date DESC, created_at DESC"
    ))
    .bind(email)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub struct GameDetails<'a> {
    pub name: &'a str,
    pub event_date: NaiveDate,
    pub budget_cents: Option<i64>,
    pub message: Option<&'a str>,
    pub organizer_name: &'a str,
}

pub async fn update_details(ex: Exec<'_>, id: GameId, d: GameDetails<'_>, now: i64) -> Result<()> {
    sqlx::query(
        "UPDATE games SET name = ?, event_date = ?, budget_cents = ?, message = ?, \
         organizer_name = ?, updated_at = ? WHERE id = ?",
    )
    .bind(d.name)
    .bind(d.event_date)
    .bind(d.budget_cents)
    .bind(d.message)
    .bind(d.organizer_name)
    .bind(now)
    .bind(id)
    .execute(ex)
    .await?;
    Ok(())
}

/// Marks the game as drawn and bumps the draw round. Returns the new round.
pub async fn update_message(
    ex: Exec<'_>,
    id: GameId,
    message: Option<&str>,
    now: i64,
) -> Result<()> {
    sqlx::query("UPDATE games SET message = ?, updated_at = ? WHERE id = ?")
        .bind(message)
        .bind(now)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn mark_drawn(ex: Exec<'_>, id: GameId, now: i64) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "UPDATE games SET status = ?, draw_round = draw_round + 1, drawn_at = ?, updated_at = ? \
         WHERE id = ? RETURNING draw_round",
    )
    .bind(GameStatus::Drawn)
    .bind(now)
    .bind(now)
    .bind(id)
    .fetch_one(ex)
    .await?;
    Ok(row.0)
}

pub async fn mark_open(ex: Exec<'_>, id: GameId, now: i64) -> Result<()> {
    sqlx::query("UPDATE games SET status = ?, drawn_at = NULL, updated_at = ? WHERE id = ?")
        .bind(GameStatus::Open)
        .bind(now)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn touch(ex: Exec<'_>, id: GameId, now: i64) -> Result<()> {
    sqlx::query("UPDATE games SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn delete(ex: Exec<'_>, id: GameId) -> Result<bool> {
    let r = sqlx::query("DELETE FROM games WHERE id = ?")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn count_created_since(ex: Exec<'_>, email: &EmailAddress, since: i64) -> Result<i64> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM games WHERE organizer_email = ? AND created_at >= ?")
            .bind(email)
            .bind(since)
            .fetch_one(ex)
            .await?;
    Ok(n)
}

/// Summary row for the admin listing.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GameSummary {
    pub id: GameId,
    pub name: String,
    pub event_date: NaiveDate,
    pub organizer_name: String,
    pub organizer_email: EmailAddress,
    pub status: GameStatus,
    pub created_at: i64,
    pub participant_count: i64,
    pub revealed_count: i64,
}

impl GameSummary {
    pub fn is_drawn(&self) -> bool {
        self.status == GameStatus::Drawn
    }
}

pub async fn admin_list(
    ex: Exec<'_>,
    search: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<GameSummary>> {
    let pattern = search.map(|s| format!("%{}%", s.trim()));
    let rows = sqlx::query_as::<_, GameSummary>(
        "SELECT g.id, g.name, g.event_date, g.organizer_name, g.organizer_email, g.status, g.created_at, \
           (SELECT COUNT(*) FROM participants p WHERE p.game_id = g.id) AS participant_count, \
           (SELECT COUNT(*) FROM participants p WHERE p.game_id = g.id AND p.revealed_at IS NOT NULL) AS revealed_count \
         FROM games g \
         WHERE (?1 IS NULL OR g.name LIKE ?1 OR g.organizer_email LIKE ?1 OR g.organizer_name LIKE ?1 OR g.id LIKE ?1) \
         ORDER BY g.created_at DESC LIMIT ?2 OFFSET ?3",
    )
    .bind(pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub async fn admin_count(ex: Exec<'_>, search: Option<&str>) -> Result<i64> {
    let pattern = search.map(|s| format!("%{}%", s.trim()));
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM games g \
         WHERE (?1 IS NULL OR g.name LIKE ?1 OR g.organizer_email LIKE ?1 OR g.organizer_name LIKE ?1 OR g.id LIKE ?1)",
    )
    .bind(pattern)
    .fetch_one(ex)
    .await?;
    Ok(n)
}

/// Deletes games whose event date is strictly before `cutoff`; participants, exclusions and
/// queued mail follow through `ON DELETE CASCADE`.
pub async fn delete_with_event_before(ex: Exec<'_>, cutoff: NaiveDate) -> Result<u64> {
    let r = sqlx::query("DELETE FROM games WHERE event_date < ?")
        .bind(cutoff)
        .execute(ex)
        .await?;
    Ok(r.rows_affected())
}

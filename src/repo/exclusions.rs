use anyhow::Result;

use super::Exec;
use crate::ids::GameId;
use crate::models::Exclusion;

pub async fn list(ex: Exec<'_>, game_id: GameId) -> Result<Vec<Exclusion>> {
    let rows = sqlx::query_as::<_, Exclusion>(
        "SELECT a_id, b_id FROM exclusions WHERE game_id = ? ORDER BY rowid",
    )
    .bind(game_id)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

/// Inserts the pair; returns false if it already existed.
pub async fn insert(ex: Exec<'_>, game_id: GameId, x: Exclusion) -> Result<bool> {
    let r = sqlx::query("INSERT OR IGNORE INTO exclusions (game_id, a_id, b_id) VALUES (?, ?, ?)")
        .bind(game_id)
        .bind(x.a_id)
        .bind(x.b_id)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn remove(ex: Exec<'_>, game_id: GameId, x: Exclusion) -> Result<bool> {
    let r = sqlx::query("DELETE FROM exclusions WHERE game_id = ? AND a_id = ? AND b_id = ?")
        .bind(game_id)
        .bind(x.a_id)
        .bind(x.b_id)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() > 0)
}

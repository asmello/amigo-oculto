use crate::models::{EmailVerification, Game, Participant};
use crate::token::{AdminToken, GameId, ParticipantId, VerificationId, ViewToken};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool},
    Row, Sqlite,
};
use std::str::FromStr;
use ulid::Ulid;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

async fn init_db(database_url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS games (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            event_date TEXT NOT NULL,
            organizer_email TEXT NOT NULL,
            admin_token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            drawn INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS participants (
            id TEXT PRIMARY KEY,
            game_id TEXT NOT NULL,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            matched_with_id TEXT,
            view_token TEXT NOT NULL UNIQUE,
            has_viewed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS email_verifications (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            code TEXT NOT NULL,
            game_name TEXT NOT NULL,
            event_date TEXT NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            verified INTEGER NOT NULL DEFAULT 0,
            attempts INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_email_verifications_email ON email_verifications(email);
        CREATE INDEX IF NOT EXISTS idx_email_verifications_code ON email_verifications(code);

        CREATE TABLE IF NOT EXISTS email_resends (
            id TEXT PRIMARY KEY,
            game_id TEXT NOT NULL,
            participant_id TEXT,
            resend_type TEXT NOT NULL,
            resent_at TEXT NOT NULL,
            FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE,
            FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_email_resends_game_id ON email_resends(game_id);
        CREATE INDEX IF NOT EXISTS idx_email_resends_participant_id ON email_resends(participant_id);
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

impl Database {
    pub async fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:///app/data/amigo_oculto.db".to_string());
        tracing::info!("connecting to database: {}", database_url);
        let pool = init_db(&database_url).await?;
        Ok(Self { pool })
    }

    pub async fn begin(&self) -> Result<Transaction> {
        Ok(Transaction {
            inner: self.pool.begin().await?,
        })
    }

    pub async fn create_game(&self, game: &Game) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO games (
                id,
                name,
                event_date,
                organizer_email,
                admin_token,
                created_at,
                drawn
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(game.id)
        .bind(&game.name)
        .bind(game.event_date)
        .bind(&game.organizer_email)
        .bind(&game.admin_token)
        .bind(game.created_at)
        .bind(game.drawn)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_game_by_id(&self, game_id: GameId) -> Result<Option<Game>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, event_date, organizer_email, admin_token, created_at, drawn
            FROM games
            WHERE id = ?
            "#,
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Game {
            id: r.get("id"),
            name: r.get("name"),
            event_date: r.get("event_date"),
            organizer_email: r.get("organizer_email"),
            admin_token: r.get("admin_token"),
            created_at: r.get("created_at"),
            drawn: r.get::<i32, _>("drawn") != 0,
        }))
    }

    pub async fn get_game_by_admin_token(&self, admin_token: &AdminToken) -> Result<Option<Game>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, event_date, organizer_email, admin_token, created_at, drawn
            FROM games
            WHERE admin_token = ?
            "#,
        )
        .bind(admin_token)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Game {
            id: r.get("id"),
            name: r.get("name"),
            event_date: r.get("event_date"),
            organizer_email: r.get("organizer_email"),
            admin_token: r.get("admin_token"),
            created_at: r.get("created_at"),
            drawn: r.get::<i32, _>("drawn") != 0,
        }))
    }

    pub async fn add_participant(&self, participant: &Participant) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO participants (
                id,
                game_id,
                name,
                email,
                matched_with_id,
                view_token,
                has_viewed,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(participant.id)
        .bind(participant.game_id)
        .bind(&participant.name)
        .bind(&participant.email)
        .bind(participant.matched_with_id)
        .bind(&participant.view_token)
        .bind(participant.has_viewed)
        .bind(participant.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_participants_by_game(&self, game_id: GameId) -> Result<Vec<Participant>> {
        let rows = sqlx::query(
            r#"
            SELECT id, game_id, name, email, matched_with_id, view_token, has_viewed, created_at
            FROM participants
            WHERE game_id = ?
            ORDER BY created_at ASC
        "#,
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Participant {
                id: r.get("id"),
                game_id: r.get("game_id"),
                name: r.get("name"),
                email: r.get("email"),
                matched_with_id: r.get("matched_with_id"),
                view_token: r.get("view_token"),
                has_viewed: r.get::<i32, _>("has_viewed") != 0,
                created_at: r.get("created_at"),
            })
            .collect())
    }

    pub async fn get_participant_by_view_token(
        &self,
        view_token: &ViewToken,
    ) -> Result<Option<Participant>> {
        let row = sqlx::query(
            r#"
        SELECT id, game_id, name, email, matched_with_id, view_token, has_viewed, created_at
        FROM participants
        WHERE view_token = ?
        "#,
        )
        .bind(view_token)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Participant {
            id: r.get("id"),
            game_id: r.get("game_id"),
            name: r.get("name"),
            email: r.get("email"),
            matched_with_id: r.get("matched_with_id"),
            view_token: r.get("view_token"),
            has_viewed: r.get::<i32, _>("has_viewed") != 0,
            created_at: r.get("created_at"),
        }))
    }

    pub async fn mark_participant_viewed(&self, participant_id: ParticipantId) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE participants
            SET has_viewed = 1
            WHERE id = ?
        "#,
        )
        .bind(participant_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_participant_by_id(
        &self,
        participant_id: ParticipantId,
    ) -> Result<Option<Participant>> {
        let row = sqlx::query(
            r#"
            SELECT id, game_id, name, email, matched_with_id, view_token, has_viewed, created_at
            FROM participants
            WHERE id = ?
        "#,
        )
        .bind(participant_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Participant {
            id: r.get("id"),
            game_id: r.get("game_id"),
            name: r.get("name"),
            email: r.get("email"),
            matched_with_id: r.get("matched_with_id"),
            view_token: r.get("view_token"),
            has_viewed: r.get::<i32, _>("has_viewed") != 0,
            created_at: r.get("created_at"),
        }))
    }

    pub async fn update_participant(
        &self,
        participant_id: ParticipantId,
        name: Option<String>,
        email: Option<String>,
    ) -> Result<()> {
        // Build dynamic update query based on what fields are provided
        if let Some(new_name) = name {
            sqlx::query(
                r#"
            UPDATE participants
            SET name = ?
            WHERE id = ?
            "#,
            )
            .bind(&new_name)
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        }

        if let Some(new_email) = email {
            sqlx::query(
                r#"
            UPDATE participants
            SET email = ?
            WHERE id = ?
            "#,
            )
            .bind(&new_email)
            .bind(participant_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn delete_game(&self, game_id: GameId) -> Result<()> {
        // CASCADE delete will automatically remove participants
        sqlx::query(
            r#"
            DELETE FROM games
            WHERE id = ?
        "#,
        )
        .bind(game_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Email verification functions
    pub async fn create_email_verification(&self, verification: &EmailVerification) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO email_verifications (id, email, code, game_name, event_date, created_at, expires_at, verified, attempts)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(verification.id)
        .bind(&verification.email)
        .bind(&verification.code)
        .bind(&verification.game_name)
        .bind(verification.event_date)
        .bind(verification.created_at)
        .bind(verification.expires_at)
        .bind(verification.verified)
        .bind(verification.attempts)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_email_verification_by_id(
        &self,
        verification_id: VerificationId,
    ) -> Result<Option<EmailVerification>> {
        let row = sqlx::query(
            r#"
        SELECT id, email, code, game_name, event_date, created_at, expires_at, verified, attempts
        FROM email_verifications
        WHERE id = ?
        "#,
        )
        .bind(verification_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| EmailVerification {
            id: r.get("id"),
            email: r.get("email"),
            code: r.get("code"),
            game_name: r.get("game_name"),
            event_date: r.get("event_date"),
            created_at: r.get("created_at"),
            expires_at: r.get("expires_at"),
            verified: r.get::<i32, _>("verified") != 0,
            attempts: r.get("attempts"),
        }))
    }

    pub async fn increment_verification_attempts(
        &self,
        verification_id: VerificationId,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE email_verifications
            SET attempts = attempts + 1
            WHERE id = ?
            "#,
        )
        .bind(verification_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_verification_as_verified(
        &self,
        verification_id: VerificationId,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE email_verifications
            SET verified = 1
            WHERE id = ?
            "#,
        )
        .bind(verification_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_recent_verifications_by_email(
        &self,
        email: &str,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
        SELECT COUNT(*) as count
        FROM email_verifications
        WHERE email = ? AND created_at > ?
        "#,
        )
        .bind(email)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    pub async fn update_verification_code(
        &self,
        verification_id: VerificationId,
        new_code: &str,
        new_expires_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE email_verifications
            SET code = ?, expires_at = ?, attempts = 0
            WHERE id = ?
            "#,
        )
        .bind(new_code)
        .bind(new_expires_at)
        .bind(verification_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cleanup_expired_verifications(&self) -> Result<u64> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            DELETE FROM email_verifications
            WHERE expires_at < ? AND verified = 0
            "#,
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // Email resend tracking functions
    pub async fn record_email_resend(
        &self,
        game_id: GameId,
        participant_id: Option<ParticipantId>,
        resend_type: &str,
    ) -> Result<()> {
        let id = Ulid::new().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO email_resends (id, game_id, participant_id, resend_type, resent_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(game_id)
        .bind(participant_id)
        .bind(resend_type)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_recent_participant_resends(
        &self,
        participant_id: ParticipantId,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM email_resends
            WHERE participant_id = ? AND resent_at > ? AND resend_type = 'individual'
            "#,
        )
        .bind(participant_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    pub async fn count_total_participant_resends(&self, participant_id: ParticipantId) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM email_resends
            WHERE participant_id = ? AND resend_type = 'individual'
            "#,
        )
        .bind(participant_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    pub async fn count_recent_bulk_resends(
        &self,
        game_id: GameId,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM email_resends
            WHERE game_id = ? AND resent_at > ? AND resend_type = 'bulk'
            "#,
        )
        .bind(game_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    pub async fn count_total_bulk_resends(&self, game_id: GameId) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM email_resends
            WHERE game_id = ? AND resend_type = 'bulk'
            "#,
        )
        .bind(game_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    pub async fn count_participants_in_game(&self, game_id: GameId) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM participants
            WHERE game_id = ?
            "#,
        )
        .bind(game_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }
}

pub struct Transaction {
    inner: sqlx::Transaction<'static, Sqlite>,
}

impl Transaction {
    pub async fn commit(self) -> Result<()> {
        self.inner
            .commit()
            .await
            .context("committing transaction")?;
        Ok(())
    }

    pub async fn get_game_by_id(&mut self, game_id: GameId) -> Result<Option<Game>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, event_date, organizer_email, admin_token, created_at, drawn
            FROM games
            WHERE id = ?
            "#,
        )
        .bind(game_id)
        .fetch_optional(&mut *self.inner)
        .await?;

        Ok(row.map(|r| Game {
            id: r.get("id"),
            name: r.get("name"),
            event_date: r.get("event_date"),
            organizer_email: r.get("organizer_email"),
            admin_token: r.get("admin_token"),
            created_at: r.get("created_at"),
            drawn: r.get::<i32, _>("drawn") != 0,
        }))
    }

    pub async fn get_participants_by_game(&mut self, game_id: GameId) -> Result<Vec<Participant>> {
        let rows = sqlx::query(
            r#"
            SELECT id, game_id, name, email, matched_with_id, view_token, has_viewed, created_at
            FROM participants
            WHERE game_id = ?
            ORDER BY created_at ASC
        "#,
        )
        .bind(game_id)
        .fetch_all(&mut *self.inner)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Participant {
                id: r.get("id"),
                game_id: r.get("game_id"),
                name: r.get("name"),
                email: r.get("email"),
                matched_with_id: r.get("matched_with_id"),
                view_token: r.get("view_token"),
                has_viewed: r.get::<i32, _>("has_viewed") != 0,
                created_at: r.get("created_at"),
            })
            .collect())
    }

    pub async fn update_participant_matches(
        &mut self,
        matches: &[(ParticipantId, ParticipantId)],
    ) -> Result<()> {
        for (participant_id, matched_with_id) in matches {
            sqlx::query(
                r#"
                UPDATE participants
                SET matched_with_id = ?
                WHERE id = ?
                "#,
            )
            .bind(matched_with_id)
            .bind(participant_id)
            .execute(&mut *self.inner)
            .await?;
        }

        Ok(())
    }

    pub async fn mark_game_as_drawn(&mut self, game_id: GameId) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE games
            SET drawn = 1
            WHERE id = ?
            "#,
        )
        .bind(game_id)
        .execute(&mut *self.inner)
        .await?;

        Ok(())
    }
}

use crate::models::{EmailVerification, Game, Participant};
use crate::token::{
    AdminSessionToken, AdminToken, EmailAddress, GameId, ParticipantId, VerificationId, ViewToken,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sqlx::{
    Row, Sqlite,
    sqlite::{SqliteConnectOptions, SqlitePool},
};
use std::str::FromStr;
use ulid::Ulid;

/// Number of days after event_date before a game is eligible for cleanup.
pub const GAME_RETENTION_DAYS: u32 = 90;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

async fn init_db(database_url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
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

        CREATE TABLE IF NOT EXISTS site_admin_password (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            password_hash TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS admin_sessions (
            id TEXT PRIMARY KEY,
            session_token TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_admin_sessions_token ON admin_sessions(session_token);
        CREATE INDEX IF NOT EXISTS idx_admin_sessions_expires ON admin_sessions(expires_at);
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
        email: Option<EmailAddress>,
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
        email: &EmailAddress,
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

    /// Delete games where event_date is more than GAME_RETENTION_DAYS in the past.
    /// Returns the number of games deleted.
    pub async fn cleanup_old_games(&self) -> Result<u64> {
        let cutoff = Utc::now().date_naive() - Duration::days(GAME_RETENTION_DAYS.into());

        // Delete related records first to handle databases created before CASCADE was added.
        // Order: email_resends (references participants) → participants → games
        sqlx::query(
            r#"
            DELETE FROM email_resends
            WHERE game_id IN (SELECT id FROM games WHERE event_date < ?)
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM participants
            WHERE game_id IN (SELECT id FROM games WHERE event_date < ?)
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        let result = sqlx::query(
            r#"
            DELETE FROM games
            WHERE event_date < ?
            "#,
        )
        .bind(cutoff)
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

    pub async fn count_total_participant_resends(
        &self,
        participant_id: ParticipantId,
    ) -> Result<i64> {
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

    pub async fn count_participants_in_game(&self, game_id: GameId) -> Result<u64> {
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

        let count: i64 = row.get("count");
        u64::try_from(count).context("converting participant count to u64")
    }

    // Site admin authentication functions

    /// Initialize the site admin password from environment variable if not set.
    /// This should be called on startup.
    pub async fn init_site_admin_password(&self) -> Result<()> {
        // Check if password is already set
        let existing = sqlx::query("SELECT id FROM site_admin_password WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;

        if existing.is_some() {
            tracing::info!("site admin password already initialized");
            return Ok(());
        }

        // Get password from environment
        let password = std::env::var("SITE_ADMIN_PASSWORD")
            .context("SITE_ADMIN_PASSWORD not set and no password in database")?;

        if password.is_empty() {
            anyhow::bail!("SITE_ADMIN_PASSWORD cannot be empty");
        }

        // Hash password
        let password_hash =
            bcrypt::hash(&password, bcrypt::DEFAULT_COST).context("hashing site admin password")?;

        // Store in database
        sqlx::query(
            r#"
            INSERT INTO site_admin_password (id, password_hash, updated_at)
            VALUES (1, ?, ?)
            "#,
        )
        .bind(&password_hash)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .context("storing site admin password")?;

        tracing::info!("site admin password initialized from environment");
        Ok(())
    }

    /// Verify the site admin password and return true if correct.
    pub async fn verify_site_admin_password(&self, password: &str) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT password_hash
            FROM site_admin_password
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .context("fetching site admin password hash")?;

        let Some(row) = row else {
            tracing::warn!("site admin password not initialized");
            return Ok(false);
        };

        let password_hash: String = row.get("password_hash");
        bcrypt::verify(password, &password_hash).context("verifying site admin password")
    }

    /// Change the site admin password. Requires the current password.
    pub async fn change_site_admin_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<bool> {
        // Verify current password
        if !self.verify_site_admin_password(current_password).await? {
            return Ok(false);
        }

        // Hash new password
        let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
            .context("hashing new site admin password")?;

        // Update in database
        sqlx::query(
            r#"
            UPDATE site_admin_password
            SET password_hash = ?, updated_at = ?
            WHERE id = 1
            "#,
        )
        .bind(&new_hash)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .context("updating site admin password")?;

        tracing::info!("site admin password changed");
        Ok(true)
    }

    /// Create a new admin session and return the session token.
    /// Sessions expire after 24 hours.
    pub async fn create_admin_session(&self) -> Result<AdminSessionToken> {
        let session_token = AdminSessionToken::generate();
        let id = Ulid::new().to_string();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(24);

        sqlx::query(
            r#"
            INSERT INTO admin_sessions (id, session_token, created_at, expires_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&session_token)
        .bind(created_at)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .context("creating admin session")?;

        Ok(session_token)
    }

    /// Validate an admin session token. Returns true if valid and not expired.
    pub async fn validate_admin_session(&self, session_token: &AdminSessionToken) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT expires_at
            FROM admin_sessions
            WHERE session_token = ?
            "#,
        )
        .bind(session_token)
        .fetch_optional(&self.pool)
        .await
        .context("fetching admin session")?;

        let Some(row) = row else {
            return Ok(false);
        };

        let expires_at: DateTime<Utc> = row.get("expires_at");
        Ok(Utc::now() < expires_at)
    }

    /// Delete an admin session (logout).
    pub async fn delete_admin_session(&self, session_token: &AdminSessionToken) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM admin_sessions
            WHERE session_token = ?
            "#,
        )
        .bind(session_token)
        .execute(&self.pool)
        .await
        .context("deleting admin session")?;

        Ok(())
    }

    /// Clean up expired admin sessions. Returns the number of sessions deleted.
    pub async fn cleanup_expired_admin_sessions(&self) -> Result<u64> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            DELETE FROM admin_sessions
            WHERE expires_at < ?
            "#,
        )
        .bind(now)
        .execute(&self.pool)
        .await
        .context("cleaning up expired admin sessions")?;

        Ok(result.rows_affected())
    }

    // Site admin game management functions

    /// Search for games by name, organizer email, or game ID.
    /// Returns paginated results ordered by created_at DESC.
    pub async fn search_games(
        &self,
        search: Option<&str>,
        limit: u32,
        offset: u64,
    ) -> Result<Vec<Game>> {
        // Convert to i64 for SQLite binding
        let limit_i64 = i64::from(limit);
        let offset_i64 = i64::try_from(offset).context("offset too large for database")?;

        let query = if let Some(search_term) = search {
            sqlx::query(
                r#"
                SELECT id, name, event_date, organizer_email, admin_token, created_at, drawn
                FROM games
                WHERE name LIKE ? OR organizer_email LIKE ? OR id LIKE ?
                ORDER BY created_at DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(format!("%{}%", search_term))
            .bind(format!("%{}%", search_term))
            .bind(format!("%{}%", search_term))
            .bind(limit_i64)
            .bind(offset_i64)
        } else {
            sqlx::query(
                r#"
                SELECT id, name, event_date, organizer_email, admin_token, created_at, drawn
                FROM games
                ORDER BY created_at DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(limit_i64)
            .bind(offset_i64)
        };

        let rows = query.fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|r| Game {
                id: r.get("id"),
                name: r.get("name"),
                event_date: r.get("event_date"),
                organizer_email: r.get("organizer_email"),
                admin_token: r.get("admin_token"),
                created_at: r.get("created_at"),
                drawn: r.get::<i32, _>("drawn") != 0,
            })
            .collect())
    }

    /// Count total games matching search criteria.
    pub async fn count_games(&self, search: Option<&str>) -> Result<u64> {
        let row = if let Some(search_term) = search {
            sqlx::query(
                r#"
                SELECT COUNT(*) as count
                FROM games
                WHERE name LIKE ? OR organizer_email LIKE ? OR id LIKE ?
                "#,
            )
            .bind(format!("%{}%", search_term))
            .bind(format!("%{}%", search_term))
            .bind(format!("%{}%", search_term))
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query("SELECT COUNT(*) as count FROM games")
                .fetch_one(&self.pool)
                .await?
        };

        let count: i64 = row.get("count");
        u64::try_from(count).context("converting game count to u64")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Game, Participant};
    use chrono::NaiveDate;

    /// Create an in-memory database for testing.
    async fn setup_test_db() -> Database {
        let pool = init_db(":memory:").await.unwrap();
        Database { pool }
    }

    /// Create a test game with a specific event_date.
    fn create_test_game(name: &str, event_date: NaiveDate) -> Game {
        Game {
            id: GameId::new(),
            name: name.to_string(),
            event_date,
            organizer_email: format!("{}@test.com", name).parse().unwrap(),
            admin_token: crate::token::AdminToken::generate(),
            created_at: Utc::now(),
            drawn: false,
        }
    }

    #[tokio::test]
    async fn test_cleanup_old_games_deletes_expired() {
        let db = setup_test_db().await;

        // Create a game with event_date > 90 days ago
        let old_date = Utc::now().date_naive() - Duration::days(100);
        let old_game = create_test_game("old_game", old_date);
        db.create_game(&old_game).await.unwrap();

        // Verify game exists
        let found = db.get_game_by_id(old_game.id).await.unwrap();
        assert!(found.is_some());

        // Run cleanup
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 1);

        // Verify game was deleted
        let found = db.get_game_by_id(old_game.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_old_games_preserves_recent() {
        let db = setup_test_db().await;

        // Create a game with event_date within retention period
        let recent_date = Utc::now().date_naive() - Duration::days(30);
        let recent_game = create_test_game("recent_game", recent_date);
        db.create_game(&recent_game).await.unwrap();

        // Run cleanup
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 0);

        // Verify game still exists
        let found = db.get_game_by_id(recent_game.id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_cleanup_old_games_preserves_future() {
        let db = setup_test_db().await;

        // Create a game with event_date in the future
        let future_date = Utc::now().date_naive() + Duration::days(30);
        let future_game = create_test_game("future_game", future_date);
        db.create_game(&future_game).await.unwrap();

        // Run cleanup
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 0);

        // Verify game still exists
        let found = db.get_game_by_id(future_game.id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_cleanup_old_games_returns_correct_count() {
        let db = setup_test_db().await;

        // Create 3 expired games
        for i in 0..3 {
            let old_date = Utc::now().date_naive() - Duration::days(100 + i);
            let game = create_test_game(&format!("old_{}", i), old_date);
            db.create_game(&game).await.unwrap();
        }

        // Create 2 recent games
        for i in 0..2 {
            let recent_date = Utc::now().date_naive() - Duration::days(10 + i);
            let game = create_test_game(&format!("recent_{}", i), recent_date);
            db.create_game(&game).await.unwrap();
        }

        // Run cleanup
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 3);
    }

    #[tokio::test]
    async fn test_cleanup_old_games_boundary() {
        let db = setup_test_db().await;

        // Create a game exactly at the boundary (should NOT be deleted)
        let boundary_date = Utc::now().date_naive() - Duration::days(GAME_RETENTION_DAYS.into());
        let boundary_game = create_test_game("boundary_game", boundary_date);
        db.create_game(&boundary_game).await.unwrap();

        // Create a game one day past the boundary (should be deleted)
        let past_boundary =
            Utc::now().date_naive() - Duration::days(GAME_RETENTION_DAYS as i64 + 1);
        let old_game = create_test_game("old_game", past_boundary);
        db.create_game(&old_game).await.unwrap();

        // Run cleanup
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 1);

        // Verify boundary game still exists
        let found = db.get_game_by_id(boundary_game.id).await.unwrap();
        assert!(found.is_some());

        // Verify old game was deleted
        let found = db.get_game_by_id(old_game.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_old_games_with_related_records() {
        let db = setup_test_db().await;

        // Create an expired game
        let old_date = Utc::now().date_naive() - Duration::days(100);
        let game = create_test_game("old_game", old_date);
        db.create_game(&game).await.unwrap();

        // Add participants
        let participant1 = Participant::new(
            game.id,
            "Alice".to_string(),
            "alice@test.com".parse().unwrap(),
        );
        let participant2 =
            Participant::new(game.id, "Bob".to_string(), "bob@test.com".parse().unwrap());
        db.add_participant(&participant1).await.unwrap();
        db.add_participant(&participant2).await.unwrap();

        // Record email resends (references both game and participants)
        db.record_email_resend(game.id, None, "admin_link")
            .await
            .unwrap();
        db.record_email_resend(game.id, Some(participant1.id), "participant_reveal")
            .await
            .unwrap();

        // Verify records exist
        assert!(db.get_game_by_id(game.id).await.unwrap().is_some());
        let participants = db.get_participants_by_game(game.id).await.unwrap();
        assert_eq!(participants.len(), 2);

        // Run cleanup - this should not fail with foreign key errors
        let deleted = db.cleanup_old_games().await.unwrap();
        assert_eq!(deleted, 1);

        // Verify game and all related records are deleted
        assert!(db.get_game_by_id(game.id).await.unwrap().is_none());
        let participants = db.get_participants_by_game(game.id).await.unwrap();
        assert!(participants.is_empty());
    }
}

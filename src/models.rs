//! Database row types and domain enums.

use chrono::NaiveDate;
use sqlx::FromRow;

use crate::email_addr::EmailAddress;
use crate::ids::{GameId, OutboxId, ParticipantId, SecretHash, VerificationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum GameStatus {
    Open,
    Drawn,
}

#[derive(Debug, Clone, FromRow)]
pub struct Game {
    pub id: GameId,
    pub name: String,
    pub event_date: NaiveDate,
    pub budget_cents: Option<i64>,
    pub message: Option<String>,
    pub organizer_name: String,
    pub organizer_email: EmailAddress,
    pub status: GameStatus,
    pub draw_round: i64,
    pub drawn_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Game {
    pub fn is_drawn(&self) -> bool {
        self.status == GameStatus::Drawn
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Participant {
    pub id: ParticipantId,
    pub game_id: GameId,
    pub name: String,
    pub email: EmailAddress,
    pub wishlist: Option<String>,
    pub receiver_id: Option<ParticipantId>,
    pub reveal_token_hash: SecretHash,
    pub revealed_at: Option<i64>,
    pub position: i64,
    pub created_at: i64,
}

impl Participant {
    pub fn has_revealed(&self) -> bool {
        self.revealed_at.is_some()
    }
}

/// A symmetric "may not draw each other" rule. Stored with `a < b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRow)]
pub struct Exclusion {
    pub a_id: ParticipantId,
    pub b_id: ParticipantId,
}

impl Exclusion {
    pub fn new(x: ParticipantId, y: ParticipantId) -> Self {
        if x <= y {
            Self { a_id: x, b_id: y }
        } else {
            Self { a_id: y, b_id: x }
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Verification {
    pub id: VerificationId,
    pub email: EmailAddress,
    pub code_hash: SecretHash,
    pub name: String,
    pub event_date: NaiveDate,
    pub budget_cents: Option<i64>,
    pub message: Option<String>,
    pub organizer_name: String,
    pub attempts: i64,
    pub expires_at: i64,
    pub consumed_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum SessionKind {
    Organizer,
    Admin,
}

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id_hash: SecretHash,
    pub email: EmailAddress,
    pub kind: SessionKind,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct LoginToken {
    pub token_hash: SecretHash,
    pub email: EmailAddress,
    pub kind: SessionKind,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum OutboxStatus {
    Queued,
    Sent,
    Failed,
}

/// Kinds of email we send. Stored as text so the organizer page can pick out invitations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum EmailKind {
    Verification,
    Welcome,
    Invite,
    Drawn,
    Recover,
    AdminLogin,
}

#[derive(Debug, Clone, FromRow)]
pub struct OutboxEmail {
    pub id: OutboxId,
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
    pub status: OutboxStatus,
    pub attempts: i64,
    pub next_attempt_at: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub sent_at: Option<i64>,
}

/// Delivery state of the latest invitation to a participant, for the organizer's dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteDelivery {
    /// No invitation queued for the current draw (should not happen after a draw).
    None,
    Queued,
    Sent,
    Failed,
}

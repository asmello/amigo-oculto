use crate::token::{AdminToken, GameId, ParticipantId, VerificationId, ViewToken};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerification {
    pub id: VerificationId,
    pub email: String,
    pub code: String,
    pub game_name: String,
    pub event_date: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub verified: bool,
    pub attempts: i32,
}

impl EmailVerification {
    pub fn new(email: String, game_name: String, event_date: NaiveDate) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let code = format!("{:06}", rng.gen_range(0..1000000));
        let created_at = Utc::now();
        let expires_at = created_at + chrono::Duration::minutes(15);

        Self {
            id: VerificationId::new(),
            email,
            code,
            game_name,
            event_date,
            created_at,
            expires_at,
            verified: false,
            attempts: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn can_attempt(&self) -> bool {
        self.attempts < 5
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: GameId,
    pub name: String,
    pub event_date: NaiveDate,
    pub organizer_email: String,
    pub admin_token: AdminToken,
    pub created_at: DateTime<Utc>,
    pub drawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: ParticipantId,
    pub game_id: GameId,
    pub name: String,
    pub email: String,
    pub matched_with_id: Option<ParticipantId>,
    pub view_token: ViewToken,
    pub has_viewed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    pub name: String,
    pub event_date: NaiveDate,
    pub organizer_email: String,
}

#[derive(Debug, Serialize)]
pub struct CreateGameResponse {
    pub game_id: GameId,
    pub admin_token: String,
}

#[derive(Debug, Deserialize)]
pub struct AddParticipantRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct AddParticipantResponse {
    pub participant_id: ParticipantId,
}

#[derive(Debug, Deserialize)]
pub struct UpdateParticipantRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GameStatusResponse {
    pub game: Game,
    pub participants: Vec<ParticipantStatus>,
}

#[derive(Debug, Serialize)]
pub struct ParticipantStatus {
    pub id: ParticipantId,
    pub name: String,
    pub email: String,
    pub has_viewed: bool,
}

#[derive(Debug, Serialize)]
pub struct RevealResponse {
    pub game_name: String,
    pub event_date: NaiveDate,
    pub your_name: String,
    pub matched_name: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestVerificationRequest {
    pub name: String,
    pub event_date: NaiveDate,
    pub organizer_email: String,
}

#[derive(Debug, Serialize)]
pub struct RequestVerificationResponse {
    pub verification_id: VerificationId,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCodeRequest {
    pub verification_id: VerificationId,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyCodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<GameId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts_remaining: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ResendVerificationRequest {
    pub verification_id: VerificationId,
}

#[derive(Debug, Serialize)]
pub struct ResendVerificationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Game {
    pub fn new(name: String, event_date: NaiveDate, organizer_email: String) -> Self {
        Self {
            id: GameId::new(),
            name,
            event_date,
            organizer_email,
            admin_token: AdminToken::generate(),
            created_at: Utc::now(),
            drawn: false,
        }
    }
}

impl Participant {
    pub fn new(game_id: GameId, name: String, email: String) -> Self {
        Self {
            id: ParticipantId::new(),
            game_id,
            name,
            email,
            matched_with_id: None,
            view_token: ViewToken::generate(),
            has_viewed: false,
            created_at: Utc::now(),
        }
    }
}

// Site admin request/response models

#[derive(Debug, Deserialize)]
pub struct SiteAdminLoginRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SiteAdminLoginResponse {
    pub session_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchGamesQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
pub struct SearchGamesResponse {
    pub games: Vec<GameSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct GameSummary {
    pub id: GameId,
    pub name: String,
    pub event_date: NaiveDate,
    pub organizer_email: String,
    pub created_at: DateTime<Utc>,
    pub drawn: bool,
    pub participant_count: i64,
}

#[derive(Debug, Serialize)]
pub struct GameDetailResponse {
    pub game: Game,
    pub participants: Vec<Participant>,
    pub participant_count: i64,
}

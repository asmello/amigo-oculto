use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use crate::token::SecureToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerification {
    pub id: String,
    pub email: String,
    pub code: String,
    pub game_name: String,
    pub event_date: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub verified: bool,
    pub attempts: i32,
}

impl EmailVerification {
    pub fn new(email: String, game_name: String, event_date: String) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let code = format!("{:06}", rng.gen_range(0..1000000));
        let created_at = Utc::now();
        let expires_at = created_at + chrono::Duration::minutes(15);
        
        Self {
            id: Ulid::new().to_string(),
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
    pub id: String,
    pub name: String,
    pub event_date: String,
    pub organizer_email: String,
    pub admin_token: SecureToken,
    pub created_at: DateTime<Utc>,
    pub drawn: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub game_id: String,
    pub name: String,
    pub email: String,
    pub matched_with_id: Option<String>,
    pub view_token: SecureToken,
    pub has_viewed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGameRequest {
    pub name: String,
    pub event_date: String,
    pub organizer_email: String,
}

#[derive(Debug, Serialize)]
pub struct CreateGameResponse {
    pub game_id: String,
    pub admin_token: String,
}

#[derive(Debug, Deserialize)]
pub struct AddParticipantRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct AddParticipantResponse {
    pub participant_id: String,
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
    pub id: String,
    pub name: String,
    pub email: String,
    pub has_viewed: bool,
}

#[derive(Debug, Serialize)]
pub struct RevealResponse {
    pub game_name: String,
    pub event_date: String,
    pub your_name: String,
    pub matched_name: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestVerificationRequest {
    pub name: String,
    pub event_date: String,
    pub organizer_email: String,
}

#[derive(Debug, Serialize)]
pub struct RequestVerificationResponse {
    pub verification_id: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCodeRequest {
    pub verification_id: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyCodeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts_remaining: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ResendVerificationRequest {
    pub verification_id: String,
}

#[derive(Debug, Serialize)]
pub struct ResendVerificationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Game {
    pub fn new(name: String, event_date: String, organizer_email: String) -> Self {
        Self {
            id: Ulid::new().to_string(),
            name,
            event_date,
            organizer_email,
            admin_token: SecureToken::generate(),
            created_at: Utc::now(),
            drawn: false,
        }
    }
}

impl Participant {
    pub fn new(game_id: String, name: String, email: String) -> Self {
        Self {
            id: Ulid::new().to_string(),
            game_id,
            name,
            email,
            matched_with_id: None,
            view_token: SecureToken::generate(),
            has_viewed: false,
            created_at: Utc::now(),
        }
    }
}
use crate::{db, email::EmailService, matching, models::*};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Maximum number of participants allowed per game to prevent abuse
const MAX_PARTICIPANTS_PER_GAME: i64 = 100;

pub struct AppState {
    pub pool: SqlitePool,
    pub email_service: EmailService,
}

#[derive(Deserialize)]
pub struct AdminQuery {
    pub admin_token: String,
}

// POST /api/games - Create a new game
pub async fn create_game(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let game = Game::new(req.name, req.event_date, req.organizer_email);

    db::create_game(&state.pool, &game).await?;

    Ok(Json(CreateGameResponse {
        game_id: game.id,
        admin_token: game.admin_token.to_string(),
    }))
}

// POST /api/games/:game_id/participants - Add a participant to a game
pub async fn add_participant(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    Json(req): Json<AddParticipantRequest>,
) -> Result<Json<AddParticipantResponse>, AppError> {
    // Check if game exists
    let game = db::get_game_by_id(&state.pool, &game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    // Check if game has already been drawn
    if game.drawn {
        return Err(AppError::BadRequest(
            "Não é possível adicionar participantes após o sorteio já ter sido realizado"
                .to_string(),
        ));
    }

    // Check participant limit to prevent abuse
    let participant_count = db::count_participants_in_game(&state.pool, &game_id).await?;
    if participant_count >= MAX_PARTICIPANTS_PER_GAME {
        return Err(AppError::BadRequest(
            format!("Limite máximo de {} participantes atingido", MAX_PARTICIPANTS_PER_GAME),
        ));
    }

    let participant = Participant::new(game_id, req.name, req.email);

    db::add_participant(&state.pool, &participant).await?;

    Ok(Json(AddParticipantResponse {
        participant_id: participant.id,
    }))
}

// POST /api/games/:game_id/draw - Generate matches and send emails
pub async fn draw_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Start a transaction to prevent race conditions
    let mut tx = state.pool.begin().await?;
    
    // Get game with lock (IMMEDIATE transaction prevents concurrent draws)
    let game = db::get_game_by_id_tx(&mut tx, &game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    // Check if already drawn
    if game.drawn {
        tx.rollback().await?;
        return Err(AppError::BadRequest(
            "O sorteio já foi realizado para este jogo".to_string(),
        ));
    }

    // Get participants
    let participants = db::get_participants_by_game_tx(&mut tx, &game_id).await?;

    if participants.len() < 2 {
        tx.rollback().await?;
        return Err(AppError::BadRequest(
            "Precisa de pelo menos 2 participantes para fazer o sorteio".to_string(),
        ));
    }

    // Generate matches
    let matches = matching::generate_matches(&participants)?;

    // Save matches and mark as drawn (all within transaction)
    db::update_participant_matches_tx(&mut tx, &matches).await?;
    db::mark_game_as_drawn_tx(&mut tx, &game_id).await?;
    
    // Commit transaction before sending emails
    tx.commit().await?;

    // Send emails to all participants
    for participant in &participants {
        if let Err(e) = state
            .email_service
            .send_participant_notification(
                &participant.name,
                &participant.email,
                &game.name,
                &game.event_date,
                &participant.view_token,
            )
            .await
        {
            tracing::error!("Failed to send email to {}: {}", participant.email, e);
        }
    }

    // Send confirmation email to organizer
    if let Err(e) = state
        .email_service
        .send_organizer_confirmation(
            &game.organizer_email,
            &game.name,
            &game.event_date,
            &game.id,
            &game.admin_token,
            participants.len(),
        )
        .await
    {
        tracing::error!("Failed to send confirmation email to organizer: {}", e);
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Sorteio realizado com sucesso! Emails enviados para todos os participantes."
    })))
}

// GET /api/games/:game_id?admin_token=xxx - Get game status (organizer view)
pub async fn get_game_status(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<GameStatusResponse>, AppError> {
    // Get game by admin token
    let game = db::get_game_by_admin_token(&state.pool, &query.admin_token)
        .await?
        .ok_or(AppError::Unauthorized(
            "Token de administrador inválido".to_string(),
        ))?;

    // Verify game_id matches
    if game.id != game_id {
        return Err(AppError::Unauthorized(
            "Token de administrador inválido para este jogo".to_string(),
        ));
    }

    // Get participants
    let participants = db::get_participants_by_game(&state.pool, &game_id).await?;

    let participant_statuses: Vec<ParticipantStatus> = participants
        .into_iter()
        .map(|p| ParticipantStatus {
            id: p.id,
            name: p.name,
            email: p.email,
            has_viewed: p.has_viewed,
        })
        .collect();

    Ok(Json(GameStatusResponse {
        game,
        participants: participant_statuses,
    }))
}

// POST /api/games/:game_id/resend-all - Resend emails to all participants
pub async fn resend_all_emails(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = db::get_game_by_admin_token(&state.pool, &query.admin_token)
        .await?
        .ok_or(AppError::Unauthorized(
            "Token de administrador inválido".to_string(),
        ))?;

    // Verify game_id matches
    if game.id != game_id {
        return Err(AppError::Unauthorized(
            "Token de administrador inválido para este jogo".to_string(),
        ));
    }

    // Check if game has been drawn
    if !game.drawn {
        return Err(AppError::BadRequest(
            "O sorteio ainda não foi realizado. Realize o sorteio antes de reenviar emails.".to_string(),
        ));
    }

    // Rate limiting: Check recent bulk resends (within last hour)
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_resends = db::count_recent_bulk_resends(&state.pool, &game_id, one_hour_ago).await?;
    if recent_resends > 0 {
        return Err(AppError::BadRequest(
            "Só é possível reenviar emails em massa uma vez por hora.".to_string(),
        ));
    }

    // Check total bulk resends (lifetime limit)
    let total_resends = db::count_total_bulk_resends(&state.pool, &game_id).await?;
    if total_resends >= 3 {
        return Err(AppError::BadRequest(
            "Limite de 3 reenvios em massa atingido.".to_string(),
        ));
    }

    // Get all participants
    let participants = db::get_participants_by_game(&state.pool, &game_id).await?;

    // Resend emails to all participants
    let mut sent_count = 0;
    let mut failed_count = 0;

    for participant in &participants {
        match state
            .email_service
            .send_participant_notification(
                &participant.name,
                &participant.email,
                &game.name,
                &game.event_date,
                &participant.view_token,
            )
            .await
        {
            Ok(_) => sent_count += 1,
            Err(e) => {
                tracing::error!("Failed to resend email to {}: {}", participant.email, e);
                failed_count += 1;
            }
        }
    }

    // Record the bulk resend
    db::record_email_resend(&state.pool, &game_id, None, "bulk").await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Emails reenviados: {} enviados, {} falharam", sent_count, failed_count),
        "sent": sent_count,
        "failed": failed_count
    })))
}

// PATCH /api/games/:game_id/participants/:participant_id - Update participant details
pub async fn update_participant(
    State(state): State<Arc<AppState>>,
    Path((game_id, participant_id)): Path<(String, String)>,
    Query(query): Query<AdminQuery>,
    Json(req): Json<UpdateParticipantRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = db::get_game_by_admin_token(&state.pool, &query.admin_token)
        .await?
        .ok_or(AppError::Unauthorized(
            "Token de administrador inválido".to_string(),
        ))?;

    // Verify game_id matches
    if game.id != game_id {
        return Err(AppError::Unauthorized(
            "Token de administrador inválido para este jogo".to_string(),
        ));
    }

    // Get participant to verify it exists and belongs to this game
    let participant = db::get_participant_by_id(&state.pool, &participant_id)
        .await?
        .ok_or(AppError::NotFound("Participante não encontrado".to_string()))?;

    if participant.game_id != game_id {
        return Err(AppError::BadRequest(
            "Participante não pertence a este jogo".to_string(),
        ));
    }

    // If game has been drawn and participant has viewed, can't edit email
    if game.drawn && participant.has_viewed {
        return Err(AppError::BadRequest(
            "Não é possível editar participante após ter visualizado o sorteio.".to_string(),
        ));
    }

    // Update participant
    db::update_participant(&state.pool, &participant_id, req.name, req.email).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Participante atualizado com sucesso"
    })))
}

// POST /api/games/:game_id/participants/:participant_id/resend - Resend email to one participant
pub async fn resend_participant_email(
    State(state): State<Arc<AppState>>,
    Path((game_id, participant_id)): Path<(String, String)>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = db::get_game_by_admin_token(&state.pool, &query.admin_token)
        .await?
        .ok_or(AppError::Unauthorized(
            "Token de administrador inválido".to_string(),
        ))?;

    // Verify game_id matches
    if game.id != game_id {
        return Err(AppError::Unauthorized(
            "Token de administrador inválido para este jogo".to_string(),
        ));
    }

    // Check if game has been drawn
    if !game.drawn {
        return Err(AppError::BadRequest(
            "O sorteio ainda não foi realizado.".to_string(),
        ));
    }

    // Get participant
    let participant = db::get_participant_by_id(&state.pool, &participant_id)
        .await?
        .ok_or(AppError::NotFound("Participante não encontrado".to_string()))?;

    // Verify participant belongs to this game
    if participant.game_id != game_id {
        return Err(AppError::BadRequest(
            "Participante não pertence a este jogo".to_string(),
        ));
    }

    // Rate limiting: Check recent individual resends (within last hour)
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_resends = db::count_recent_participant_resends(&state.pool, &participant_id, one_hour_ago).await?;
    if recent_resends > 0 {
        return Err(AppError::BadRequest(
            "Só é possível reenviar email para este participante uma vez por hora.".to_string(),
        ));
    }

    // Check total individual resends (lifetime limit)
    let total_resends = db::count_total_participant_resends(&state.pool, &participant_id).await?;
    if total_resends >= 3 {
        return Err(AppError::BadRequest(
            "Limite de 3 reenvios para este participante atingido.".to_string(),
        ));
    }

    // Resend email
    state
        .email_service
        .send_participant_notification(
            &participant.name,
            &participant.email,
            &game.name,
            &game.event_date,
            &participant.view_token,
        )
        .await?;

    // Record the individual resend
    db::record_email_resend(&state.pool, &game_id, Some(&participant_id), "individual").await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Email reenviado para {}", participant.email)
    })))
}

// DELETE /api/games/:game_id - Delete a game (requires admin_token)
pub async fn delete_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<String>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = db::get_game_by_admin_token(&state.pool, &query.admin_token)
        .await?
        .ok_or(AppError::Unauthorized(
            "Token de administrador inválido".to_string(),
        ))?;

    // Verify game_id matches
    if game.id != game_id {
        return Err(AppError::Unauthorized(
            "Token de administrador inválido para este jogo".to_string(),
        ));
    }

    // Delete game (participants will be cascade deleted)
    db::delete_game(&state.pool, &game_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Jogo excluído com sucesso"
    })))
}

// GET /api/reveal/:view_token - View your match
pub async fn reveal_match(
    State(state): State<Arc<AppState>>,
    Path(view_token): Path<String>,
) -> Result<Json<RevealResponse>, AppError> {
    // Get participant by view token
    let participant = db::get_participant_by_view_token(&state.pool, &view_token)
        .await?
        .ok_or(AppError::NotFound("Link inválido ou expirado".to_string()))?;

    // Check if game has been drawn
    let game = db::get_game_by_id(&state.pool, &participant.game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    if !game.drawn {
        return Err(AppError::BadRequest(
            "O sorteio ainda não foi realizado. Aguarde o organizador finalizar o sorteio."
                .to_string(),
        ));
    }

    // Get matched participant
    let matched_with_id = participant
        .matched_with_id
        .as_ref()
        .ok_or(AppError::InternalError(
            "Sorteio ainda não foi realizado".to_string(),
        ))?;

    let matched_participant = db::get_participant_by_id(&state.pool, matched_with_id)
        .await?
        .ok_or(AppError::InternalError(
            "Participante sorteado não encontrado".to_string(),
        ))?;

    // Mark as viewed
    if !participant.has_viewed {
        db::mark_participant_viewed(&state.pool, &participant.id).await?;
    }

    Ok(Json(RevealResponse {
        game_name: game.name,
        event_date: game.event_date,
        your_name: participant.name,
        matched_name: matched_participant.name,
    }))
}

/// POST /api/verifications/request - Request email verification code
/// 
/// Initiates the email verification process by generating a 6-digit code
/// and sending it to the organizer's email. The code expires in 15 minutes.
/// 
/// Rate limiting: Maximum 3 verification requests per email per hour.
pub async fn request_verification(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RequestVerificationRequest>,
) -> Result<Json<RequestVerificationResponse>, AppError> {
    // Rate limiting: Check if email has requested too many verifications recently
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_count = db::count_recent_verifications_by_email(&state.pool, &req.organizer_email, one_hour_ago).await?;
    
    if recent_count >= 3 {
        return Err(AppError::BadRequest(
            "Muitas tentativas de verificação. Tente novamente em 1 hora.".to_string(),
        ));
    }

    // Create verification
    let verification = EmailVerification::new(
        req.organizer_email.clone(),
        req.name.clone(),
        req.event_date.clone(),
    );

    // Store in database
    db::create_email_verification(&state.pool, &verification).await?;

    // Send verification email
    if let Err(e) = state
        .email_service
        .send_verification_code(
            &verification.email,
            &verification.game_name,
            &verification.code,
        )
        .await
    {
        tracing::error!("Failed to send verification email: {}", e);
        return Err(AppError::InternalError(
            "Erro ao enviar email de verificação".to_string(),
        ));
    }

    Ok(Json(RequestVerificationResponse {
        verification_id: verification.id,
    }))
}

/// POST /api/verifications/verify - Verify code and create game
/// 
/// Validates the 6-digit verification code and creates the game if successful.
/// On success, sends an admin welcome email with the admin panel link.
/// 
/// Maximum 5 attempts per verification before it must be requested again.
pub async fn verify_code(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyCodeRequest>,
) -> Result<Json<VerifyCodeResponse>, AppError> {
    // Get verification
    let verification = db::get_email_verification_by_id(&state.pool, &req.verification_id)
        .await?
        .ok_or(AppError::NotFound(
            "Verificação não encontrada".to_string(),
        ))?;

    // Check if already verified
    if verification.verified {
        return Err(AppError::BadRequest(
            "Esta verificação já foi usada".to_string(),
        ));
    }

    // Check if expired
    if verification.is_expired() {
        return Ok(Json(VerifyCodeResponse {
            success: false,
            game_id: None,
            admin_token: None,
            error: Some("Código expirado. Solicite um novo código.".to_string()),
            attempts_remaining: None,
        }));
    }

    // Check attempts
    if !verification.can_attempt() {
        return Ok(Json(VerifyCodeResponse {
            success: false,
            game_id: None,
            admin_token: None,
            error: Some("Número máximo de tentativas excedido. Solicite um novo código.".to_string()),
            attempts_remaining: Some(0),
        }));
    }

    // Verify code
    if verification.code != req.code {
        // Increment attempts
        db::increment_verification_attempts(&state.pool, &req.verification_id).await?;
        
        let attempts_remaining = 5 - (verification.attempts + 1);
        return Ok(Json(VerifyCodeResponse {
            success: false,
            game_id: None,
            admin_token: None,
            error: Some(format!("Código incorreto. {} tentativas restantes.", attempts_remaining)),
            attempts_remaining: Some(attempts_remaining),
        }));
    }

    // Code is correct! Create the game
    let game = Game::new(
        verification.game_name.clone(),
        verification.event_date.clone(),
        verification.email.clone(),
    );

    db::create_game(&state.pool, &game).await?;

    // Mark verification as verified
    db::mark_verification_as_verified(&state.pool, &req.verification_id).await?;

    // Send admin welcome email
    if let Err(e) = state
        .email_service
        .send_admin_welcome(
            &game.organizer_email,
            &game.name,
            &game.event_date,
            &game.id,
            &game.admin_token,
        )
        .await
    {
        tracing::error!("Failed to send admin welcome email: {}", e);
        // Don't fail the request if email fails
    }

    Ok(Json(VerifyCodeResponse {
        success: true,
        game_id: Some(game.id),
        admin_token: Some(game.admin_token.to_string()),
        error: None,
        attempts_remaining: None,
    }))
}

/// POST /api/verifications/resend - Resend verification code
/// 
/// Generates and sends a new 6-digit verification code, resetting the attempt counter.
/// The new code expires in 15 minutes. Rate limiting applies (max 3 per hour).
pub async fn resend_verification(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResendVerificationRequest>,
) -> Result<Json<ResendVerificationResponse>, AppError> {
    // Get verification
    let verification = db::get_email_verification_by_id(&state.pool, &req.verification_id)
        .await?
        .ok_or(AppError::NotFound(
            "Verificação não encontrada".to_string(),
        ))?;

    // Check if already verified
    if verification.verified {
        return Ok(Json(ResendVerificationResponse {
            success: false,
            error: Some("Esta verificação já foi usada".to_string()),
        }));
    }

    // Rate limiting: Check recent verifications for this email
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_count = db::count_recent_verifications_by_email(&state.pool, &verification.email, one_hour_ago).await?;
    
    if recent_count >= 3 {
        return Ok(Json(ResendVerificationResponse {
            success: false,
            error: Some("Muitas tentativas de verificação. Tente novamente em 1 hora.".to_string()),
        }));
    }

    // Generate new code
    let new_code = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!("{:06}", rng.gen_range(0..1000000))
    };
    let new_expires_at = Utc::now() + Duration::minutes(15);

    // Update verification with new code
    db::update_verification_code(&state.pool, &req.verification_id, &new_code, new_expires_at).await?;

    // Send new verification email
    if let Err(e) = state
        .email_service
        .send_verification_code(
            &verification.email,
            &verification.game_name,
            &new_code,
        )
        .await
    {
        tracing::error!("Failed to resend verification email: {}", e);
        return Ok(Json(ResendVerificationResponse {
            success: false,
            error: Some("Erro ao enviar email de verificação".to_string()),
        }));
    }

    Ok(Json(ResendVerificationResponse {
        success: true,
        error: None,
    }))
}

// Error handling
#[derive(Debug)]
pub enum AppError {
    Database(sqlx::Error),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    InternalError(String),
    Anyhow(anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Anyhow(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Erro interno do servidor".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::Anyhow(e) => {
                tracing::error!("Error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
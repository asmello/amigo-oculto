use crate::{
    db::Database,
    email::EmailService,
    matching,
    models::*,
    site_admin_auth::SiteAdminLayer,
    staging_auth::StagingAuthLayer,
    token::{AdminToken, GameId, ParticipantId, ViewToken},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, get_service, patch, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tower_http::{
    cors::{self, CorsLayer},
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

/// Maximum number of participants allowed per game to prevent abuse
const MAX_PARTICIPANTS_PER_GAME: u64 = 100;

pub fn make(db: Database, email_service: EmailService) -> Router {
    let state = Arc::new(AppState {
        db: db.clone(),
        email_service,
    });

    // Site admin protected routes (require authentication)
    let site_admin_protected = Router::new()
        .route("/change-password", post(site_admin_change_password))
        .route("/games", get(site_admin_search_games))
        .route(
            "/games/{game_id}",
            get(site_admin_get_game).delete(site_admin_delete_game),
        )
        .layer(SiteAdminLayer::new(db))
        .with_state(state.clone());

    let api_routes = Router::new()
        .route("/verifications/request", post(request_verification))
        .route("/verifications/verify", post(verify_code))
        .route("/verifications/resend", post(resend_verification))
        .route("/games", post(create_game))
        .route("/games/{game_id}/participants", post(add_participant))
        .route("/games/{game_id}/draw", post(draw_game))
        .route("/games/{game_id}/resend-all", post(resend_all_emails))
        .route(
            "/games/{game_id}/participants/{participant_id}/resend",
            post(resend_participant_email),
        )
        .route(
            "/games/{game_id}/participants/{participant_id}",
            patch(update_participant),
        )
        .route("/games/{game_id}", get(get_game_status).delete(delete_game))
        .route("/reveal/{view_token}", get(reveal_match))
        // Site admin public routes (no authentication required)
        .route("/site-admin/login", post(site_admin_login))
        // Site admin protected routes
        .nest("/site-admin", site_admin_protected)
        .with_state(state);

    let cors = CorsLayer::new()
        .allow_origin(cors::Any)
        .allow_methods(cors::Any)
        .allow_headers(cors::Any);

    let static_base_dir = std::path::PathBuf::from(
        std::env::var("STATIC_DIR").unwrap_or_else(|_| "/app/public".into()),
    );

    let static_dir = ServeDir::new(&static_base_dir)
        .not_found_service(ServeFile::new(static_base_dir.join("index.html")));

    // Staging protection (enabled if STAGING_SECRET is set)
    let staging_auth = StagingAuthLayer::from_env();
    if staging_auth.is_enabled() {
        tracing::info!("staging authentication enabled (X-Staging-Secret header required)");
    }

    Router::new()
        .nest("/api", api_routes)
        .fallback_service(get_service(static_dir).handle_error(|error| async move {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("static file error: {error}"),
            )
        }))
        .layer(staging_auth)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

pub struct AppState {
    pub db: Database,
    pub email_service: EmailService,
}

#[derive(Deserialize)]
pub struct AdminQuery {
    pub admin_token: AdminToken,
}

// POST /api/games - Create a new game
pub async fn create_game(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let game = Game::new(req.name, req.event_date, req.organizer_email);
    state.db.create_game(&game).await?;

    Ok(Json(CreateGameResponse {
        game_id: game.id,
        admin_token: game.admin_token.to_string(),
    }))
}

// POST /api/games/:game_id/participants - Add a participant to a game
pub async fn add_participant(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<AddParticipantRequest>,
) -> Result<Json<AddParticipantResponse>, AppError> {
    // Check if game exists
    let game = state
        .db
        .get_game_by_id(game_id)
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
    let participant_count = state.db.count_participants_in_game(game_id).await?;
    if participant_count >= MAX_PARTICIPANTS_PER_GAME {
        return Err(AppError::BadRequest(format!(
            "Limite máximo de {} participantes atingido",
            MAX_PARTICIPANTS_PER_GAME
        )));
    }

    let participant = Participant::new(game_id, req.name, req.email);
    state.db.add_participant(&participant).await?;

    Ok(Json(AddParticipantResponse {
        participant_id: participant.id,
    }))
}

// POST /api/games/:game_id/draw - Generate matches and send emails
pub async fn draw_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Start a transaction to prevent race conditions
    let mut tx = state.db.begin().await?;

    // Get game with lock (IMMEDIATE transaction prevents concurrent draws)
    let game = tx
        .get_game_by_id(game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    // Check if already drawn
    if game.drawn {
        return Err(AppError::BadRequest(
            "O sorteio já foi realizado para este jogo".to_string(),
        ));
    }

    // Get participants
    let participants = tx.get_participants_by_game(game_id).await?;
    if participants.len() < 2 {
        return Err(AppError::BadRequest(
            "Precisa de pelo menos 2 participantes para fazer o sorteio".to_string(),
        ));
    }

    // Generate matches
    let matches = matching::generate_matches(&participants)?;

    // Save matches and mark as drawn (all within transaction)
    tx.update_participant_matches(&matches).await?;
    tx.mark_game_as_drawn(game_id).await?;

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
                game.event_date,
                &participant.view_token,
            )
            .await
        {
            tracing::error!("failed to send email to {}: {}", participant.email, e);
        }
    }

    // Send confirmation email to organizer
    if let Err(e) = state
        .email_service
        .send_organizer_confirmation(
            &game.organizer_email,
            &game.name,
            game.event_date,
            game.id,
            &game.admin_token,
            participants.len(),
        )
        .await
    {
        tracing::error!("failed to send confirmation email to organizer: {}", e);
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Sorteio realizado com sucesso! Emails enviados para todos os participantes."
    })))
}

// GET /api/games/:game_id?admin_token=xxx - Get game status (organizer view)
pub async fn get_game_status(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<GameStatusResponse>, AppError> {
    // Get game by admin token
    let game = state
        .db
        .get_game_by_admin_token(&query.admin_token)
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
    let participants = state.db.get_participants_by_game(game_id).await?;

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
    Path(game_id): Path<GameId>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = state
        .db
        .get_game_by_admin_token(&query.admin_token)
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
            "O sorteio ainda não foi realizado. Realize o sorteio antes de reenviar emails."
                .to_string(),
        ));
    }

    // Rate limiting: Check recent bulk resends (within last hour)
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_resends = state
        .db
        .count_recent_bulk_resends(game_id, one_hour_ago)
        .await?;
    if recent_resends > 0 {
        return Err(AppError::BadRequest(
            "Só é possível reenviar emails em massa uma vez por hora.".to_string(),
        ));
    }

    // Check total bulk resends (lifetime limit)
    let total_resends = state.db.count_total_bulk_resends(game_id).await?;
    if total_resends >= 3 {
        return Err(AppError::BadRequest(
            "Limite de 3 reenvios em massa atingido.".to_string(),
        ));
    }

    // Get all participants
    let participants = state.db.get_participants_by_game(game_id).await?;

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
                game.event_date,
                &participant.view_token,
            )
            .await
        {
            Ok(_) => sent_count += 1,
            Err(e) => {
                tracing::error!("failed to resend email to {}: {}", participant.email, e);
                failed_count += 1;
            }
        }
    }

    // Record the bulk resend
    state.db.record_email_resend(game_id, None, "bulk").await?;

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
    Path((game_id, participant_id)): Path<(GameId, ParticipantId)>,
    Query(query): Query<AdminQuery>,
    Json(req): Json<UpdateParticipantRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = state
        .db
        .get_game_by_admin_token(&query.admin_token)
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
    let participant = state
        .db
        .get_participant_by_id(participant_id)
        .await?
        .ok_or(AppError::NotFound(
            "Participante não encontrado".to_string(),
        ))?;

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
    state
        .db
        .update_participant(participant_id, req.name, req.email)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Participante atualizado com sucesso"
    })))
}

// POST /api/games/:game_id/participants/:participant_id/resend - Resend email to one participant
pub async fn resend_participant_email(
    State(state): State<Arc<AppState>>,
    Path((game_id, participant_id)): Path<(GameId, ParticipantId)>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = state
        .db
        .get_game_by_admin_token(&query.admin_token)
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
    let participant = state
        .db
        .get_participant_by_id(participant_id)
        .await?
        .ok_or(AppError::NotFound(
            "Participante não encontrado".to_string(),
        ))?;

    // Verify participant belongs to this game
    if participant.game_id != game_id {
        return Err(AppError::BadRequest(
            "Participante não pertence a este jogo".to_string(),
        ));
    }

    // Rate limiting: Check recent individual resends (within last hour)
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_resends = state
        .db
        .count_recent_participant_resends(participant_id, one_hour_ago)
        .await?;
    if recent_resends > 0 {
        return Err(AppError::BadRequest(
            "Só é possível reenviar email para este participante uma vez por hora.".to_string(),
        ));
    }

    // Check total individual resends (lifetime limit)
    let total_resends = state
        .db
        .count_total_participant_resends(participant_id)
        .await?;
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
            game.event_date,
            &participant.view_token,
        )
        .await?;

    // Record the individual resend
    state
        .db
        .record_email_resend(game_id, Some(participant_id), "individual")
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Email reenviado para {}", participant.email)
    })))
}

// DELETE /api/games/:game_id - Delete a game (requires admin_token)
pub async fn delete_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Query(query): Query<AdminQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify admin token
    let game = state
        .db
        .get_game_by_admin_token(&query.admin_token)
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
    state.db.delete_game(game_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Jogo excluído com sucesso"
    })))
}

// GET /api/reveal/:view_token - View your match
pub async fn reveal_match(
    State(state): State<Arc<AppState>>,
    Path(view_token): Path<ViewToken>,
) -> Result<Json<RevealResponse>, AppError> {
    // Get participant by view token
    let participant = state
        .db
        .get_participant_by_view_token(&view_token)
        .await?
        .ok_or(AppError::NotFound("Link inválido ou expirado".to_string()))?;

    // Check if game has been drawn
    let game = state
        .db
        .get_game_by_id(participant.game_id)
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
        .ok_or(AppError::InternalError(
            "Sorteio ainda não foi realizado".to_string(),
        ))?;

    let matched_participant = state
        .db
        .get_participant_by_id(matched_with_id)
        .await?
        .ok_or(AppError::InternalError(
            "Participante sorteado não encontrado".to_string(),
        ))?;

    // Mark as viewed
    if !participant.has_viewed {
        state.db.mark_participant_viewed(participant.id).await?;
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
    let recent_count = state
        .db
        .count_recent_verifications_by_email(&req.organizer_email, one_hour_ago)
        .await?;

    if recent_count >= 3 {
        return Err(AppError::BadRequest(
            "Muitas tentativas de verificação. Tente novamente em 1 hora.".to_string(),
        ));
    }

    // Create verification
    let verification = EmailVerification::new(
        req.organizer_email.clone(),
        req.name.clone(),
        req.event_date,
    );

    // Store in database
    state.db.create_email_verification(&verification).await?;

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
        tracing::error!("failed to send verification email: {}", e);
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
    let verification = state
        .db
        .get_email_verification_by_id(req.verification_id)
        .await?
        .ok_or(AppError::NotFound("Verificação não encontrada".to_string()))?;

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
            error: Some(
                "Número máximo de tentativas excedido. Solicite um novo código.".to_string(),
            ),
            attempts_remaining: Some(0),
        }));
    }

    // Verify code
    if verification.code != req.code {
        // Increment attempts
        state
            .db
            .increment_verification_attempts(req.verification_id)
            .await?;

        let attempts_remaining = 5 - (verification.attempts + 1);
        return Ok(Json(VerifyCodeResponse {
            success: false,
            game_id: None,
            admin_token: None,
            error: Some(format!(
                "Código incorreto. {} tentativas restantes.",
                attempts_remaining
            )),
            attempts_remaining: Some(attempts_remaining),
        }));
    }

    // Code is correct! Create the game
    let game = Game::new(
        verification.game_name.clone(),
        verification.event_date,
        verification.email.clone(),
    );

    state.db.create_game(&game).await?;

    // Mark verification as verified
    state
        .db
        .mark_verification_as_verified(req.verification_id)
        .await?;

    // Send admin welcome email
    if let Err(e) = state
        .email_service
        .send_admin_welcome(
            &game.organizer_email,
            &game.name,
            game.event_date,
            game.id,
            &game.admin_token,
        )
        .await
    {
        tracing::error!("failed to send admin welcome email: {}", e);
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
    let verification = state
        .db
        .get_email_verification_by_id(req.verification_id)
        .await?
        .ok_or(AppError::NotFound("Verificação não encontrada".to_string()))?;

    // Check if already verified
    if verification.verified {
        return Ok(Json(ResendVerificationResponse {
            success: false,
            error: Some("Esta verificação já foi usada".to_string()),
        }));
    }

    // Rate limiting: Check recent verifications for this email
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent_count = state
        .db
        .count_recent_verifications_by_email(&verification.email, one_hour_ago)
        .await?;

    if recent_count >= 3 {
        return Ok(Json(ResendVerificationResponse {
            success: false,
            error: Some(
                "Muitas tentativas de verificação. Tente novamente em 1 hora.".to_string(),
            ),
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
    state
        .db
        .update_verification_code(req.verification_id, &new_code, new_expires_at)
        .await?;

    // Send new verification email
    if let Err(e) = state
        .email_service
        .send_verification_code(&verification.email, &verification.game_name, &new_code)
        .await
    {
        tracing::error!("failed to resend verification email: {}", e);
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

// Site admin endpoints

/// POST /api/site-admin/login - Authenticate with password and get session token
pub async fn site_admin_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SiteAdminLoginRequest>,
) -> Result<Json<SiteAdminLoginResponse>, AppError> {
    // Verify password
    let valid = state
        .db
        .verify_site_admin_password(&req.password)
        .await?;

    if !valid {
        tracing::warn!("failed site admin login attempt");
        return Err(AppError::Unauthorized(
            "Senha incorreta".to_string(),
        ));
    }

    // Create session
    let session_token = state.db.create_admin_session().await?;
    let expires_at = Utc::now() + Duration::hours(24);

    tracing::info!("site admin logged in");

    Ok(Json(SiteAdminLoginResponse {
        session_token: session_token.to_string(),
        expires_at,
    }))
}

/// POST /api/site-admin/change-password - Change the site admin password
pub async fn site_admin_change_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Validate new password
    if req.new_password.is_empty() {
        return Err(AppError::BadRequest("Nova senha não pode ser vazia".to_string()));
    }

    if req.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "Nova senha deve ter pelo menos 8 caracteres".to_string(),
        ));
    }

    // Change password
    let success = state
        .db
        .change_site_admin_password(&req.current_password, &req.new_password)
        .await?;

    if !success {
        return Err(AppError::Unauthorized(
            "Senha atual incorreta".to_string(),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Senha alterada com sucesso"
    })))
}

/// GET /api/site-admin/games - Search and list games with pagination
pub async fn site_admin_search_games(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchGamesQuery>,
) -> Result<Json<SearchGamesResponse>, AppError> {
    // Validate pagination parameters
    let limit = query.limit.clamp(1, 100);
    let offset = query.offset.max(0);

    // Search games
    let games = state
        .db
        .search_games(query.search.as_deref(), limit, offset)
        .await?;

    // Get total count
    let total = state.db.count_games(query.search.as_deref()).await?;

    // Build response with participant counts
    let mut game_summaries = Vec::new();
    for game in games {
        let participant_count = state.db.count_participants_in_game(game.id).await?;
        game_summaries.push(GameSummary {
            id: game.id,
            name: game.name,
            event_date: game.event_date,
            organizer_email: game.organizer_email,
            created_at: game.created_at,
            drawn: game.drawn,
            participant_count,
        });
    }

    Ok(Json(SearchGamesResponse {
        games: game_summaries,
        total,
        limit,
        offset,
    }))
}

/// GET /api/site-admin/games/:game_id - Get full game details including admin token
pub async fn site_admin_get_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<GameDetailResponse>, AppError> {
    let game = state
        .db
        .get_game_by_id(game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    let participants = state.db.get_participants_by_game(game_id).await?;
    let participant_count = participants.len() as u64;

    Ok(Json(GameDetailResponse {
        game,
        participants,
        participant_count,
    }))
}

/// DELETE /api/site-admin/games/:game_id - Permanently delete a game
pub async fn site_admin_delete_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify game exists
    let game = state
        .db
        .get_game_by_id(game_id)
        .await?
        .ok_or(AppError::NotFound("Jogo não encontrado".to_string()))?;

    // Delete game (participants will be cascade deleted)
    state.db.delete_game(game_id).await?;

    tracing::info!(
        "site admin deleted game {} ({}) organized by {}",
        game_id,
        game.name,
        game.organizer_email
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Jogo excluído com sucesso"
    })))
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
                tracing::error!("database error: {}", e);
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
                tracing::error!("error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}

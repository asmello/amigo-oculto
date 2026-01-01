//! Site admin authentication middleware.
//!
//! Validates session tokens from the Authorization header to authenticate
//! site administrators. Session tokens are obtained via the login endpoint.

use crate::{db::Database, token::AdminSessionToken};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Middleware function that validates site admin authentication.
///
/// Checks for a valid Bearer token in the Authorization header and validates
/// it against active sessions in the database.
pub async fn require_site_admin(
    State(db): State<Database>,
    request: Request,
    next: Next,
) -> Response {
    // Get Authorization header
    let auth_header = match request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
    {
        Some(header) => header,
        None => {
            tracing::warn!("site admin request missing authorization header");
            return unauthorized_response("Missing authorization token");
        }
    };

    // Parse "Bearer <token>"
    let token_str = match auth_header.strip_prefix("Bearer ") {
        Some(token) => token,
        None => {
            tracing::warn!("site admin request has invalid authorization format");
            return unauthorized_response("Invalid authorization format");
        }
    };

    // Parse token
    let session_token: AdminSessionToken = match token_str.parse() {
        Ok(token) => token,
        Err(_) => {
            tracing::warn!("site admin request has malformed token");
            return unauthorized_response("Invalid token");
        }
    };

    // Validate session
    match db.validate_admin_session(&session_token).await {
        Ok(true) => {
            // Authentication successful, proceed with request
            next.run(request).await
        }
        Ok(false) => {
            tracing::warn!("site admin request with invalid or expired session");
            unauthorized_response("Invalid or expired token")
        }
        Err(e) => {
            tracing::error!("failed to validate site admin session: {}", e);
            internal_error_response()
        }
    }
}

fn unauthorized_response(message: &str) -> Response {
    let body = Json(serde_json::json!({
        "error": message
    }));

    (StatusCode::UNAUTHORIZED, body).into_response()
}

fn internal_error_response() -> Response {
    let body = Json(serde_json::json!({
        "error": "Internal server error"
    }));

    (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
}

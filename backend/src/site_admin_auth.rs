//! Site admin authentication middleware.
//!
//! Validates session tokens from the Authorization header to authenticate
//! site administrators. Session tokens are obtained via the login endpoint.

use crate::{db::Database, token::AdminSessionToken};
use axum::{
    Json,
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Extractor for the authenticated admin session token.
///
/// This can only be used in handlers behind the `require_site_admin` middleware,
/// which inserts the validated token into request extensions.
#[derive(Debug, Clone)]
pub struct AuthenticatedAdmin(pub AdminSessionToken);

impl<S> FromRequestParts<S> for AuthenticatedAdmin
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AdminSessionToken>()
            .cloned()
            .map(AuthenticatedAdmin)
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": "Missing authentication context"
                    })),
                )
            })
    }
}

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
            // Authentication successful, store token in extensions and proceed
            let mut request = request;
            request.extensions_mut().insert(session_token);
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

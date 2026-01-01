//! Site admin authentication extractor.
//!
//! Validates session tokens from the Authorization header to authenticate
//! site administrators. Session tokens are obtained via the login endpoint.

use crate::token::AdminSessionToken;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

/// Extractor that validates site admin authentication.
///
/// Use this in route handlers that require site admin access:
/// ```
/// async fn admin_only_route(
///     SiteAdmin: SiteAdmin,
///     // ... other extractors
/// ) -> Result<impl IntoResponse, AppError> {
///     // This code only runs if authentication succeeded
/// }
/// ```
pub struct SiteAdmin;

impl<S> FromRequestParts<S> for SiteAdmin
where
    S: Send + Sync,
    Arc<crate::routes::AppState>: FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract database from app state
        let app_state: Arc<crate::routes::AppState> = FromRef::from_ref(state);

        // Get Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        // Parse "Bearer <token>"
        let token_str = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidFormat)?;

        // Parse token
        let session_token: AdminSessionToken = token_str
            .parse()
            .map_err(|_| AuthError::InvalidToken)?;

        // Validate session
        let valid = app_state
            .db
            .validate_admin_session(&session_token)
            .await
            .map_err(|e| {
                tracing::error!("failed to validate admin session: {}", e);
                AuthError::DatabaseError
            })?;

        if !valid {
            return Err(AuthError::InvalidToken);
        }

        Ok(SiteAdmin)
    }
}

#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidFormat,
    InvalidToken,
    DatabaseError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing authorization token"),
            AuthError::InvalidFormat => (StatusCode::UNAUTHORIZED, "Invalid authorization format"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::DatabaseError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            ),
        };

        let body = Json(serde_json::json!({
            "error": message
        }));

        (status, body).into_response()
    }
}

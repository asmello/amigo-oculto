//! Site admin authentication middleware.
//!
//! Validates session tokens from the Authorization header to authenticate
//! site administrators. Session tokens are obtained via the login endpoint.

use crate::{db::Database, token::AdminSessionToken};
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    Json,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Tower Layer that wraps services with site admin authentication.
#[derive(Clone)]
pub struct SiteAdminLayer {
    db: Database,
}

impl SiteAdminLayer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

impl<S> Layer<S> for SiteAdminLayer {
    type Service = SiteAdminService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SiteAdminService {
            inner,
            db: self.db.clone(),
        }
    }
}

/// Tower Service that validates site admin authentication header.
#[derive(Clone)]
pub struct SiteAdminService<S> {
    inner: S,
    db: Database,
}

impl<S> Service<Request<Body>> for SiteAdminService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let db = self.db.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Get Authorization header
            let auth_header = match req
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
            {
                Some(header) => header,
                None => {
                    tracing::warn!("site admin request missing authorization header");
                    return Ok(unauthorized_response("Missing authorization token"));
                }
            };

            // Parse "Bearer <token>"
            let token_str = match auth_header.strip_prefix("Bearer ") {
                Some(token) => token,
                None => {
                    tracing::warn!("site admin request has invalid authorization format");
                    return Ok(unauthorized_response("Invalid authorization format"));
                }
            };

            // Parse token
            let session_token: AdminSessionToken = match token_str.parse() {
                Ok(token) => token,
                Err(_) => {
                    tracing::warn!("site admin request has malformed token");
                    return Ok(unauthorized_response("Invalid token"));
                }
            };

            // Validate session
            match db.validate_admin_session(&session_token).await {
                Ok(true) => {
                    // Authentication successful, proceed with request
                    inner.call(req).await
                }
                Ok(false) => {
                    tracing::warn!("site admin request with invalid or expired session");
                    Ok(unauthorized_response("Invalid or expired token"))
                }
                Err(e) => {
                    tracing::error!("failed to validate site admin session: {}", e);
                    Ok(internal_error_response())
                }
            }
        })
    }
}

fn unauthorized_response(message: &str) -> Response<Body> {
    let body = Json(serde_json::json!({
        "error": message
    }));

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body.0).unwrap()))
        .unwrap()
}

fn internal_error_response() -> Response<Body> {
    let body = Json(serde_json::json!({
        "error": "Internal server error"
    }));

    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&body.0).unwrap()))
        .unwrap()
}

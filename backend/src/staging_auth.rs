//! Staging environment protection middleware.
//!
//! Validates that requests include a secret header injected by Cloudflare.
//! This blocks direct access to the fly.dev domain while allowing
//! proxied requests through your Cloudflare-managed domain.
//!
//! Disabled when STAGING_SECRET is not set (local dev, production).

use axum::{
    body::Body,
    http::{Method, Request, Response, StatusCode},
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Name of the header that contains the secret token.
///
/// This should be set by the proxy so that requests will be accepted by the
/// server. Without it, the server will produce a 403 response.
///
/// Note that we can't use the `CF-` prefix as that's reserved for Cloudflare's
/// own headers.
const HEADER_NAME: &str = "X-Staging-Secret";

/// Configuration for staging authentication.
#[derive(Clone)]
pub struct StagingAuthConfig {
    secret: Option<String>,
}

impl StagingAuthConfig {
    /// Load configuration from environment variables.
    /// If STAGING_SECRET is not set or empty, authentication is disabled.
    pub fn from_env() -> Self {
        Self {
            secret: std::env::var("STAGING_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }

    /// Returns true if staging authentication is enabled.
    pub fn is_enabled(&self) -> bool {
        self.secret.is_some()
    }
}

/// Tower Layer that wraps services with staging authentication.
#[derive(Clone)]
pub struct StagingAuthLayer {
    config: StagingAuthConfig,
}

impl StagingAuthLayer {
    pub fn from_env() -> Self {
        Self {
            config: StagingAuthConfig::from_env(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }
}

impl<S> Layer<S> for StagingAuthLayer {
    type Service = StagingAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        StagingAuthService {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Tower Service that validates staging authentication header.
#[derive(Clone)]
pub struct StagingAuthService<S> {
    inner: S,
    config: StagingAuthConfig,
}

impl<S> Service<Request<Body>> for StagingAuthService<S>
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
        // Allow CORS preflight requests
        if req.method() == Method::OPTIONS {
            return Box::pin(self.inner.call(req));
        }

        // If no secret configured, pass through (disabled)
        let Some(expected_secret) = &self.config.secret else {
            return Box::pin(self.inner.call(req));
        };

        macro_rules! handle_error {
            ($cause:literal) => {{
                let client_ip = req
                    .headers()
                    .get("x-forwarded-for")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("unknown");

                tracing::warn!(
                    "staging auth denied: {} {HEADER_NAME} header from {client_ip}",
                    $cause
                );

                return Box::pin(async move { Ok(forbidden_response()) });
            }};
        }

        let Some(value) = req.headers().get(HEADER_NAME) else {
            handle_error!("missing")
        };

        let Ok(value) = value.to_str() else {
            handle_error!("incorrectly encoded")
        };

        if value == expected_secret.as_str() {
            Box::pin(self.inner.call(req))
        } else {
            handle_error!("invalid")
        }
    }
}

fn forbidden_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"error":"Access denied"}"#))
        .unwrap()
}

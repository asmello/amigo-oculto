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
        // If no secret configured, pass through (disabled)
        let Some(expected_secret) = &self.config.secret else {
            let future = self.inner.call(req);
            return Box::pin(future);
        };

        // Allow CORS preflight requests
        if req.method() == Method::OPTIONS {
            let future = self.inner.call(req);
            return Box::pin(future);
        }

        // Validate the header
        let header_value = req.headers().get(HEADER_NAME).and_then(|v| v.to_str().ok());

        if header_value == Some(expected_secret.as_str()) {
            // Valid header, proceed
            let future = self.inner.call(req);
            Box::pin(future)
        } else {
            // Missing or invalid header
            let client_ip = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown");

            tracing::warn!(
                "Staging auth denied: missing/invalid {} header from {}",
                HEADER_NAME,
                client_ip
            );

            Box::pin(async move { Ok(forbidden_response()) })
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

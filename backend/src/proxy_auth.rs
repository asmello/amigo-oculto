//! Proxy authentication middleware.
//!
//! Validates that requests include a secret header injected by Cloudflare.
//! This blocks direct access to the fly.dev domain while allowing
//! proxied requests through your Cloudflare-managed domain.
//!
//! Disabled when PROXY_SECRET is not set (local development).

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
const HEADER_NAME: &str = "X-Proxy-Secret";

/// Configuration for proxy authentication.
#[derive(Clone)]
pub struct ProxyAuthConfig {
    secret: Option<String>,
}

impl ProxyAuthConfig {
    /// Load configuration from environment variables.
    /// If PROXY_SECRET is not set or empty, authentication is disabled.
    pub fn from_env() -> Self {
        Self {
            secret: std::env::var("PROXY_SECRET").ok().filter(|s| !s.is_empty()),
        }
    }

    /// Returns true if proxy authentication is enabled.
    pub fn is_enabled(&self) -> bool {
        self.secret.is_some()
    }
}

/// Tower Layer that wraps services with proxy authentication.
#[derive(Clone)]
pub struct ProxyAuthLayer {
    config: ProxyAuthConfig,
}

impl ProxyAuthLayer {
    pub fn from_env() -> Self {
        Self {
            config: ProxyAuthConfig::from_env(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }
}

impl<S> Layer<S> for ProxyAuthLayer {
    type Service = ProxyAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ProxyAuthService {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Tower Service that validates proxy authentication header.
#[derive(Clone)]
pub struct ProxyAuthService<S> {
    inner: S,
    config: ProxyAuthConfig,
}

impl<S> Service<Request<Body>> for ProxyAuthService<S>
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
                    "proxy auth denied: {} {HEADER_NAME} header from {client_ip}",
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

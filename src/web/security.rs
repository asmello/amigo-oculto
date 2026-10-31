//! Security middleware: response headers, cross-site POST rejection, per-IP throttling, and
//! log redaction.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::AppState;
use super::error::AppError;

/// Adds the standard hardening headers to every response.
pub async fn headers(req: Request, next: Next) -> Response {
    let https = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"));
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let set = |h: &mut HeaderMap, name: &'static str, value: &'static str| {
        h.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    };
    set(
        h,
        "content-security-policy",
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
         script-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; \
         base-uri 'self'; object-src 'none'",
    );
    set(h, "referrer-policy", "no-referrer");
    set(h, "x-content-type-options", "nosniff");
    set(h, "x-frame-options", "DENY");
    set(
        h,
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=()",
    );
    if https {
        set(
            h,
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        );
    }
    res
}

/// Rejects cross-site form posts and throttles POSTs per client IP.
pub async fn guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.method() == Method::POST {
        if is_cross_site(req.headers(), &state.cfg.origin()) {
            tracing::warn!("rejected cross-site POST");
            return (StatusCode::FORBIDDEN, "cross-site request rejected").into_response();
        }
        let ip = client_ip(req.headers(), state.cfg.trust_proxy);
        if !state.limiter.allow(&ip) {
            tracing::warn!(ip = %ip, "POST rate limit hit");
            return AppError::TooManyRequests.into_response();
        }
    }
    next.run(req).await
}

/// Modern browsers send `Sec-Fetch-Site`; older ones at least send `Origin` on form posts.
/// Combined with `SameSite=Lax` cookies this is enough CSRF protection without tokens.
fn is_cross_site(headers: &HeaderMap, origin: &str) -> bool {
    if let Some(site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        return site.eq_ignore_ascii_case("cross-site");
    }
    match headers.get("origin").and_then(|v| v.to_str().ok()) {
        Some(o) if o != "null" => !o.eq_ignore_ascii_case(origin),
        _ => false,
    }
}

/// Client IP as a string, from the proxy header when trusted. Used only for throttling.
pub fn client_ip(headers: &HeaderMap, trust_proxy: bool) -> String {
    if trust_proxy {
        for name in ["cf-connecting-ip", "x-forwarded-for"] {
            if let Some(v) = headers.get(name).and_then(|v| v.to_str().ok()) {
                let first = v.split(',').next().unwrap_or("").trim();
                if !first.is_empty() {
                    return first.to_owned();
                }
            }
        }
    }
    "direct".to_owned()
}

/// Token-bucket limiter keyed by IP: `burst` requests, refilling `per_minute`.
pub struct IpLimiter {
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
    burst: f64,
    per_minute: f64,
}

impl Default for IpLimiter {
    fn default() -> Self {
        Self::new(40.0, 20.0)
    }
}

impl IpLimiter {
    pub fn new(burst: f64, per_minute: f64) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            burst,
            per_minute,
        }
    }

    pub fn allow(&self, key: &str) -> bool {
        let mut buckets = self.buckets.lock().expect("limiter poisoned");
        let now = Instant::now();
        if buckets.len() > 10_000 {
            buckets.retain(|_, (tokens, at)| {
                *tokens + now.duration_since(*at).as_secs_f64() * self.per_minute / 60.0
                    < self.burst
            });
        }
        let entry = buckets.entry(key.to_owned()).or_insert((self.burst, now));
        let refill = now.duration_since(entry.1).as_secs_f64() * self.per_minute / 60.0;
        entry.0 = (entry.0 + refill).min(self.burst);
        entry.1 = now;
        if entry.0 >= 1.0 {
            entry.0 -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Replaces secret path segments with a placeholder so request logs never contain tokens.
pub fn redact_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut redact_next = false;
    for (i, seg) in path.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        if redact_next {
            out.push_str("[token]");
            redact_next = false;
        } else {
            out.push_str(seg);
        }
        if matches!(seg, "revelar" | "entrar") {
            redact_next = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secret_segments() {
        assert_eq!(redact_path("/revelar/abc123"), "/revelar/[token]");
        assert_eq!(
            redact_path("/revelar/abc123/dicas"),
            "/revelar/[token]/dicas"
        );
        assert_eq!(redact_path("/entrar/xyz"), "/entrar/[token]");
        assert_eq!(redact_path("/jogo/01J"), "/jogo/01J");
        assert_eq!(redact_path("/"), "/");
    }

    #[test]
    fn limiter_blocks_after_burst() {
        let l = IpLimiter::new(3.0, 60.0);
        assert!(l.allow("a"));
        assert!(l.allow("a"));
        assert!(l.allow("a"));
        assert!(!l.allow("a"));
        assert!(l.allow("b"));
    }

    #[test]
    fn cross_site_detection() {
        let mut h = HeaderMap::new();
        assert!(!is_cross_site(&h, "https://x.app"));
        h.insert("origin", HeaderValue::from_static("https://x.app"));
        assert!(!is_cross_site(&h, "https://x.app"));
        h.insert("origin", HeaderValue::from_static("https://evil.app"));
        assert!(is_cross_site(&h, "https://x.app"));
        h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(!is_cross_site(&h, "https://x.app"));
        h.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(is_cross_site(&h, "https://x.app"));
    }
}

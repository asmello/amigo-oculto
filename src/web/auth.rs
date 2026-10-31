//! Sessions (cookie `sid` → hashed row in `sessions`) and login links (`/entrar/{token}`).

use anyhow::Result;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use super::AppState;
use super::error::AppError;
use crate::config::Config;
use crate::db;
use crate::email_addr::EmailAddress;
use crate::ids::{Secret, SecretHash};
use crate::models::{LoginToken, Session, SessionKind};
use crate::repo;

pub const SESSION_COOKIE: &str = "sid";
pub const ORGANIZER_SESSION_SECS: i64 = 90 * 24 * 3600;
pub const ADMIN_SESSION_SECS: i64 = 12 * 3600;
pub const ORGANIZER_LINK_SECS: i64 = 30 * 24 * 3600;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id_hash: SecretHash,
    pub email: EmailAddress,
    pub kind: SessionKind,
    pub is_admin: bool,
}

impl From<(Session, &Config)> for SessionInfo {
    fn from((s, cfg): (Session, &Config)) -> Self {
        let is_admin = s.kind == SessionKind::Admin && cfg.is_admin(&s.email);
        Self {
            id_hash: s.id_hash,
            email: s.email,
            kind: s.kind,
            is_admin,
        }
    }
}

/// The current session, if any. Never rejects.
pub struct MaybeSession(pub Option<SessionInfo>);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .unwrap_or_else(|never| match never {});
        let Some(cookie) = jar.get(SESSION_COOKIE) else {
            return Ok(Self(None));
        };
        let Ok(secret) = cookie.value().parse::<Secret>() else {
            return Ok(Self(None));
        };
        let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
        let session = repo::auth::get_session(&mut conn, &secret.hash(), db::now()).await?;
        Ok(Self(
            session.map(|s| SessionInfo::from((s, state.cfg.as_ref()))),
        ))
    }
}

/// An organizer session; otherwise redirects to the "get my link" page.
pub struct Organizer(pub SessionInfo);

impl FromRequestParts<AppState> for Organizer {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let MaybeSession(session) = MaybeSession::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        match session {
            Some(s) if s.kind == SessionKind::Organizer => Ok(Self(s)),
            _ => {
                let next = parts.uri.path().to_owned();
                let target = format!("/recuperar?next={}", urlencode(&next));
                Err(Redirect::to(&target).into_response())
            }
        }
    }
}

/// An admin session; otherwise redirects to the admin login form.
pub struct Admin(pub SessionInfo);

impl FromRequestParts<AppState> for Admin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let MaybeSession(session) = MaybeSession::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        match session {
            Some(s) if s.is_admin => Ok(Self(s)),
            _ => Err(Redirect::to(super::admin::PANEL_PATH).into_response()),
        }
    }
}

// Every cookie here is `Secure`, unconditionally: production is HTTPS-only, and Chromium and
// Firefox still accept Secure cookies on http://localhost for development (Safari does not —
// use another browser or local TLS). A literal `true` is also what static analysis checks.

/// Creates a session row and returns the cookie to set.
pub async fn start_session(
    conn: repo::Exec<'_>,
    email: &EmailAddress,
    kind: SessionKind,
) -> Result<Cookie<'static>> {
    let secret = Secret::generate();
    let now = db::now();
    let ttl = match kind {
        SessionKind::Organizer => ORGANIZER_SESSION_SECS,
        SessionKind::Admin => ADMIN_SESSION_SECS,
    };
    repo::auth::insert_session(
        conn,
        &Session {
            id_hash: secret.hash(),
            email: email.clone(),
            kind,
            created_at: now,
            expires_at: now + ttl,
        },
    )
    .await?;
    Ok(Cookie::build((SESSION_COOKIE, secret.as_str().to_owned()))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(ttl))
        .build())
}

pub fn clear_session_cookie() -> Cookie<'static> {
    Cookie::build(SESSION_COOKIE)
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .build()
}

/// Issues a login link token and returns the secret to embed in the URL.
pub async fn issue_login_token(
    conn: repo::Exec<'_>,
    email: &EmailAddress,
    kind: SessionKind,
    ttl_secs: i64,
) -> Result<Secret> {
    let secret = Secret::generate();
    let now = db::now();
    repo::auth::insert_login_token(
        conn,
        &LoginToken {
            token_hash: secret.hash(),
            email: email.clone(),
            kind,
            expires_at: now + ttl_secs,
            created_at: now,
        },
    )
    .await?;
    Ok(secret)
}

/// Absolute `/entrar/{token}` URL, optionally redirecting to `next` after login.
pub fn login_url(state: &AppState, secret: &Secret, next: Option<&str>) -> String {
    let mut url = state.url(&format!("/entrar/{}", secret.as_str()));
    if let Some(next) = next {
        url.push_str("?next=");
        url.push_str(&urlencode(next));
    }
    url
}

/// Only accepts local absolute paths so a login link can never bounce somewhere else.
pub fn safe_next(next: Option<&str>, default: &str) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.contains('\\') => n.to_owned(),
        _ => default.to_owned(),
    }
}

pub fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

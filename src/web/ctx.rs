//! Per-page context shared by every template (asset version, flash message, session info) and
//! the flash-message cookie.

use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Datelike;

use super::auth::SessionInfo;

pub const ASSET_VERSION: &str = env!("ASSET_VERSION");
const FLASH_COOKIE: &str = "flash";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashKind {
    Ok,
    Error,
    Info,
}

impl FlashKind {
    pub fn class(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Flash {
    pub kind: FlashKind,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PageCtx {
    pub asset_version: &'static str,
    pub flash: Option<Flash>,
    pub year: i32,
    pub session: Option<SessionInfo>,
}

impl PageCtx {
    pub fn new(flash: Option<Flash>, session: Option<SessionInfo>) -> Self {
        Self {
            asset_version: ASSET_VERSION,
            flash,
            year: chrono::Utc::now().year(),
            session,
        }
    }

    /// Context with no request information (error pages).
    pub fn bare() -> Self {
        Self::new(None, None)
    }

    pub fn is_admin(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.is_admin)
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.is_some()
    }
}

/// Builds the cookie that carries a one-shot message to the next page.
pub fn set_flash(jar: CookieJar, kind: FlashKind, text: impl Into<String>) -> CookieJar {
    let tag = match kind {
        FlashKind::Ok => "o",
        FlashKind::Error => "e",
        FlashKind::Info => "i",
    };
    let value = format!("{tag}{}", URL_SAFE_NO_PAD.encode(text.into()));
    jar.add(
        Cookie::build((FLASH_COOKIE, value))
            .path("/")
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Lax)
            .max_age(time_duration_minutes(5))
            .build(),
    )
}

/// Reads and clears the flash cookie.
pub fn take_flash(jar: CookieJar) -> (CookieJar, Option<Flash>) {
    let Some(cookie) = jar.get(FLASH_COOKIE) else {
        return (jar, None);
    };
    let value = cookie.value().to_owned();
    let jar = jar.remove(
        Cookie::build(FLASH_COOKIE)
            .path("/")
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Lax)
            .build(),
    );
    let (tag, payload) = value.split_at(usize::from(!value.is_empty()));
    let kind = match tag {
        "o" => FlashKind::Ok,
        "e" => FlashKind::Error,
        "i" => FlashKind::Info,
        _ => return (jar, None),
    };
    let text = URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .and_then(|b| String::from_utf8(b).ok());
    (jar, text.map(|text| Flash { kind, text }))
}

fn time_duration_minutes(m: i64) -> time::Duration {
    time::Duration::minutes(m)
}

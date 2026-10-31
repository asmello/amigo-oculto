//! Static pages, health check and 404.

use askama::Template;
use askama_web::WebTemplate;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;

use super::AppState;
use super::auth::MaybeSession;
use super::ctx::{PageCtx, take_flash};
use super::error::AppError;

#[derive(Template, WebTemplate)]
#[template(path = "privacy.html")]
pub struct PrivacyPage {
    pub ctx: PageCtx,
    pub retention_days: u32,
    pub contact: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "terms.html")]
pub struct TermsPage {
    pub ctx: PageCtx,
    pub contact: String,
}

const CONTACT: &str = "contato@amigoocultosimples.app";

pub async fn privacy(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    jar: CookieJar,
) -> Response {
    let (jar, flash) = take_flash(jar);
    (
        jar,
        PrivacyPage {
            ctx: PageCtx::new(flash, session),
            retention_days: state.cfg.game_retention_days,
            contact: CONTACT.to_owned(),
        },
    )
        .into_response()
}

pub async fn terms(MaybeSession(session): MaybeSession, jar: CookieJar) -> Response {
    let (jar, flash) = take_flash(jar);
    (
        jar,
        TermsPage {
            ctx: PageCtx::new(flash, session),
            contact: CONTACT.to_owned(),
        },
    )
        .into_response()
}

pub async fn healthz(State(state): State<AppState>) -> Result<Response, AppError> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok((StatusCode::OK, [("cache-control", "no-store")], "ok").into_response())
}

pub async fn not_found() -> Response {
    AppError::not_found("Essa página não existe. Confira o endereço ou volte ao início.")
        .into_response()
}

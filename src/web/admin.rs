//! Site administration: magic-link login for the configured admin emails, a list of games
//! (no tokens, no matches) and deletion.
//!
//! The panel lives at [`PANEL_PATH`], not `/admin`: the Cloudflare WAF in front of production
//! blocks `/admin` at the edge as a scanner probe (custom rule "Block suspicious probes"), and
//! keeping that block means scanners never reach the app. Routes are registered in `web::mod`.

use askama::Template;
use askama_web::WebTemplate;
use axum::Form;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use super::AppState;
use super::auth::{self, Admin, MaybeSession};
use super::ctx::{FlashKind, PageCtx, set_flash, take_flash};
use super::error::AppError;
use super::security::client_ip;
use crate::db;
use crate::email_addr::EmailAddress;
use crate::fmt::filters;

/// Where the admin panel lives. The sub-routes `/entrar` and `/jogos/{id}/excluir` hang off it.
pub const PANEL_PATH: &str = "/painel";
use crate::ids::GameId;
use crate::mail::compose::{self, ADMIN_LINK_MINUTES};
use crate::models::SessionKind;
use crate::repo::{self, games::GameSummary};

const PAGE_SIZE: i64 = 25;

#[derive(Debug, Default, Deserialize)]
pub struct AdminQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
}

#[derive(Template, WebTemplate)]
#[template(path = "admin.html")]
pub struct AdminPage {
    pub ctx: PageCtx,
    pub logged_in: bool,
    pub sent: bool,
    pub q: String,
    pub q_enc: String,
    pub games: Vec<GameSummary>,
    pub total: i64,
    pub page: i64,
    pub pages: i64,
    pub queued_emails: i64,
}

fn login_page(ctx: PageCtx, sent: bool) -> AdminPage {
    AdminPage {
        ctx,
        logged_in: false,
        sent,
        q: String::new(),
        q_enc: String::new(),
        games: vec![],
        total: 0,
        page: 1,
        pages: 1,
        queued_emails: 0,
    }
}

pub async fn index(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    Query(q): Query<AdminQuery>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let (jar, flash) = take_flash(jar);
    let Some(session) = session.filter(|s| s.is_admin) else {
        return Ok((jar, login_page(PageCtx::new(flash, None), false)).into_response());
    };
    let search = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let page = q.page.unwrap_or(1).max(1);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let total = repo::games::admin_count(&mut conn, search).await?;
    let pages = (total + PAGE_SIZE - 1).max(PAGE_SIZE) / PAGE_SIZE;
    let games =
        repo::games::admin_list(&mut conn, search, PAGE_SIZE, (page - 1) * PAGE_SIZE).await?;
    let queued_emails = repo::outbox::count_queued(&mut conn).await?;
    Ok((
        jar,
        AdminPage {
            ctx: PageCtx::new(flash, Some(session)),
            logged_in: true,
            sent: false,
            q: search.unwrap_or("").to_owned(),
            q_enc: auth::urlencode(search.unwrap_or("")),
            games,
            total,
            page,
            pages,
            queued_emails,
        },
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
pub struct LinkForm {
    #[serde(default)]
    pub email: String,
}

pub async fn request_link(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<LinkForm>,
) -> Result<Response, AppError> {
    // Whatever happens, answer the same way: the login form never confirms who the admins are.
    let sent = login_page(PageCtx::new(None, session), true);
    let Ok(email) = form.email.parse::<EmailAddress>() else {
        return Ok((jar, sent).into_response());
    };
    let now = db::now();
    let ip = client_ip(&headers, state.cfg.trust_proxy);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let allowed = repo::rate::allow(&mut conn, &format!("admin_ip:{ip}"), 5, 3600, now).await?
        && repo::rate::allow(&mut conn, &format!("admin:{email}"), 3, 3600, now).await?;
    if !allowed || !state.cfg.is_admin(&email) {
        tracing::warn!(
            allowed,
            "admin link requested for non-admin or throttled email"
        );
        return Ok((jar, sent).into_response());
    }
    let secret = auth::issue_login_token(
        &mut conn,
        &email,
        SessionKind::Admin,
        ADMIN_LINK_MINUTES * 60,
    )
    .await?;
    drop(conn);
    let url = auth::login_url(&state, &secret, Some(PANEL_PATH));
    state
        .enqueue(&compose::admin_login(&state.site, &email, &url)?)
        .await?;
    Ok((jar, sent).into_response())
}

pub async fn delete_game(
    State(state): State<AppState>,
    Admin(_session): Admin,
    Path(id): Path<GameId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let name = repo::games::get(&mut conn, id).await?.map(|g| g.name);
    let deleted = repo::games::delete(&mut conn, id).await?;
    let jar = if deleted {
        tracing::info!(game = %id, "admin deleted game");
        set_flash(
            jar,
            FlashKind::Ok,
            format!("Sorteio “{}” excluído.", name.unwrap_or_default()),
        )
    } else {
        set_flash(jar, FlashKind::Error, "Esse sorteio não existe mais.")
    };
    Ok((jar, Redirect::to(PANEL_PATH)).into_response())
}

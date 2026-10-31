//! Public pages: landing + game creation (with email verification), link recovery, login
//! links, the organizer's game list and logout.

use askama::Template;
use askama_web::WebTemplate;
use axum::Form;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use chrono::{Duration, NaiveDate};
use serde::Deserialize;

use super::AppState;
use super::auth::{self, MaybeSession, Organizer, SessionInfo};
use super::ctx::{FlashKind, PageCtx, set_flash, take_flash};
use super::error::AppError;
use super::security::client_ip;
use crate::db;
use crate::email_addr::EmailAddress;
use crate::fmt::{self, filters};
use crate::ids::{GameId, Secret, VerificationCode, VerificationId};
use crate::mail::compose::{self, LinkItem, VERIFICATION_MINUTES};
use crate::models::{Game, GameStatus, SessionKind, Verification};
use crate::repo;

pub const MAX_GAMES_PER_DAY: i64 = 5;
const MAX_CODE_ATTEMPTS: i64 = 5;

// ---------------------------------------------------------------------------------------------
// Landing + create
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CreateForm {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub event_date: String,
    #[serde(default)]
    pub budget: String,
    #[serde(default)]
    pub organizer_name: String,
    #[serde(default)]
    pub organizer_email: String,
}

struct ValidCreate {
    name: String,
    event_date: NaiveDate,
    budget_cents: Option<i64>,
    organizer_name: String,
    organizer_email: EmailAddress,
}

impl CreateForm {
    fn validate(&self) -> Result<ValidCreate, Vec<String>> {
        let mut errors = Vec::new();
        let name = clean(&self.name, 80);
        if name.chars().count() < 2 {
            errors.push("Dê um nome ao evento (pelo menos 2 letras).".to_owned());
        }
        let today = fmt::today();
        let event_date = match NaiveDate::parse_from_str(self.event_date.trim(), "%Y-%m-%d") {
            Ok(d) if d < today => {
                errors.push("A data da troca já passou.".to_owned());
                None
            }
            Ok(d) if d > today + Duration::days(730) => {
                errors.push("A data da troca está longe demais (máximo 2 anos).".to_owned());
                None
            }
            Ok(d) => Some(d),
            Err(_) => {
                errors.push("Informe a data da troca de presentes.".to_owned());
                None
            }
        };
        let budget_cents = if self.budget.trim().is_empty() {
            None
        } else {
            match fmt::parse_money(&self.budget) {
                Some(c) if (100..=10_000_000).contains(&c) => Some(c),
                _ => {
                    errors.push(
                        "Valor sugerido inválido (use só números, ex.: 80 ou 80,50).".to_owned(),
                    );
                    None
                }
            }
        };
        let organizer_name = clean(&self.organizer_name, 60);
        if organizer_name.chars().count() < 2 {
            errors.push("Diga seu nome: ele aparece nos convites.".to_owned());
        }
        let organizer_email = match self.organizer_email.parse::<EmailAddress>() {
            Ok(e) => Some(e),
            Err(_) => {
                errors.push("Confira o seu email.".to_owned());
                None
            }
        };
        match (errors.is_empty(), event_date, organizer_email) {
            (true, Some(event_date), Some(organizer_email)) => Ok(ValidCreate {
                name,
                event_date,
                budget_cents,
                organizer_name,
                organizer_email,
            }),
            _ => Err(errors),
        }
    }
}

/// Trims, collapses inner whitespace and truncates to `max` characters.
pub fn clean(s: &str, max: usize) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

/// Trims and truncates multi-line text, keeping line breaks.
pub fn clean_text(s: &str, max: usize) -> Option<String> {
    let t: String = s.replace("\r\n", "\n").trim().chars().take(max).collect();
    if t.is_empty() { None } else { Some(t) }
}

#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
pub struct HomePage {
    pub ctx: PageCtx,
    pub form: CreateForm,
    pub errors: Vec<String>,
    pub min_date: String,
}

fn home_page(ctx: PageCtx, form: CreateForm, errors: Vec<String>) -> HomePage {
    HomePage {
        ctx,
        form,
        errors,
        min_date: fmt::today().format("%Y-%m-%d").to_string(),
    }
}

pub async fn index(MaybeSession(session): MaybeSession, jar: CookieJar) -> Response {
    let (jar, flash) = take_flash(jar);
    (
        jar,
        home_page(PageCtx::new(flash, session), CreateForm::default(), vec![]),
    )
        .into_response()
}

pub async fn create(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<CreateForm>,
) -> Result<Response, AppError> {
    let valid = match form.validate() {
        Ok(v) => v,
        Err(errors) => {
            return Ok((jar, home_page(PageCtx::new(None, session), form, errors)).into_response());
        }
    };

    let now = db::now();
    let ip = client_ip(&headers, state.cfg.trust_proxy);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let email_ok = repo::rate::allow(
        &mut conn,
        &format!("verify:{}", valid.organizer_email),
        3,
        3600,
        now,
    )
    .await?;
    let ip_ok = repo::rate::allow(&mut conn, &format!("create_ip:{ip}"), 10, 3600, now).await?;
    if !email_ok || !ip_ok {
        let errors = vec!["Muitos códigos pedidos para este email na última hora. Confira sua caixa de entrada (e o spam) ou tente mais tarde.".to_owned()];
        return Ok((jar, home_page(PageCtx::new(None, session), form, errors)).into_response());
    }
    let recent =
        repo::games::count_created_since(&mut conn, &valid.organizer_email, now - 86_400).await?;
    if recent >= MAX_GAMES_PER_DAY {
        let errors = vec![format!(
            "Limite de {MAX_GAMES_PER_DAY} sorteios por dia para o mesmo email."
        )];
        return Ok((jar, home_page(PageCtx::new(None, session), form, errors)).into_response());
    }

    let id = VerificationId::new();
    let code = VerificationCode::generate();
    let verification = Verification {
        id,
        email: valid.organizer_email.clone(),
        code_hash: code.hash(id),
        name: valid.name.clone(),
        event_date: valid.event_date,
        budget_cents: valid.budget_cents,
        message: None,
        organizer_name: valid.organizer_name.clone(),
        attempts: 0,
        expires_at: now + VERIFICATION_MINUTES * 60,
        consumed_at: None,
        created_at: now,
    };
    repo::verifications::insert(&mut conn, &verification).await?;
    drop(conn);

    let email = compose::verification(
        &state.site,
        &valid.organizer_email,
        &valid.organizer_name,
        &valid.name,
        code,
    )?;
    state.enqueue(&email).await?;

    Ok(Redirect::to(&format!("/verificar/{id}")).into_response())
}

// ---------------------------------------------------------------------------------------------
// Verification code
// ---------------------------------------------------------------------------------------------

#[derive(Template, WebTemplate)]
#[template(path = "verify.html")]
pub struct VerifyPage {
    pub ctx: PageCtx,
    pub id: VerificationId,
    pub email: String,
    pub game_name: String,
    pub error: Option<String>,
    pub expired: bool,
    pub attempts_left: i64,
    pub minutes: i64,
}

async fn load_verification(state: &AppState, id: VerificationId) -> Result<Verification, AppError> {
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::verifications::get(&mut conn, id)
        .await?
        .filter(|v| v.consumed_at.is_none())
        .ok_or_else(|| {
            AppError::not_found(
                "Esse pedido de verificação não existe mais. Comece de novo na página inicial.",
            )
        })
}

fn verify_page(ctx: PageCtx, v: &Verification, error: Option<String>) -> VerifyPage {
    VerifyPage {
        ctx,
        id: v.id,
        email: v.email.to_string(),
        game_name: v.name.clone(),
        error,
        expired: v.expires_at <= db::now(),
        attempts_left: (MAX_CODE_ATTEMPTS - v.attempts).max(0),
        minutes: VERIFICATION_MINUTES,
    }
}

pub async fn verify_form(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    Path(id): Path<VerificationId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let v = load_verification(&state, id).await?;
    let (jar, flash) = take_flash(jar);
    Ok((jar, verify_page(PageCtx::new(flash, session), &v, None)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct VerifyForm {
    #[serde(default)]
    pub code: String,
}

pub async fn verify(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    Path(id): Path<VerificationId>,
    jar: CookieJar,
    Form(form): Form<VerifyForm>,
) -> Result<Response, AppError> {
    let v = load_verification(&state, id).await?;
    let now = db::now();
    if v.expires_at <= now {
        return Ok((
            jar,
            verify_page(
                PageCtx::new(None, session),
                &v,
                Some("O código expirou. Peça um novo.".into()),
            ),
        )
            .into_response());
    }
    if v.attempts >= MAX_CODE_ATTEMPTS {
        return Ok((
            jar,
            verify_page(
                PageCtx::new(None, session),
                &v,
                Some("Muitas tentativas. Peça um novo código.".into()),
            ),
        )
            .into_response());
    }
    let Ok(code) = form.code.parse::<VerificationCode>() else {
        return Ok((
            jar,
            verify_page(
                PageCtx::new(None, session),
                &v,
                Some("Digite os 6 dígitos do código.".into()),
            ),
        )
            .into_response());
    };

    if !constant_time_eq(code.hash(id).as_str(), v.code_hash.as_str()) {
        let attempts = {
            let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
            repo::verifications::record_attempt(&mut conn, id).await?
        };
        let left = (MAX_CODE_ATTEMPTS - attempts).max(0);
        let msg = if left == 0 {
            "Código incorreto. Peça um novo código para continuar.".to_owned()
        } else {
            format!(
                "Código incorreto. Você ainda tem {left} tentativa{}.",
                if left == 1 { "" } else { "s" }
            )
        };
        let mut v = v;
        v.attempts = attempts;
        return Ok((jar, verify_page(PageCtx::new(None, session), &v, Some(msg))).into_response());
    }

    // Code accepted: create the game, consume the verification, log the organizer in.
    let game = Game {
        id: GameId::new(),
        name: v.name.clone(),
        event_date: v.event_date,
        budget_cents: v.budget_cents,
        message: v.message.clone(),
        organizer_name: v.organizer_name.clone(),
        organizer_email: v.email.clone(),
        status: GameStatus::Open,
        draw_round: 0,
        drawn_at: None,
        created_at: now,
        updated_at: now,
    };
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    repo::games::insert(&mut tx, &game).await?;
    repo::verifications::consume(&mut tx, id, now).await?;
    let cookie =
        auth::start_session(&mut tx, &game.organizer_email, SessionKind::Organizer).await?;
    let login = auth::issue_login_token(
        &mut tx,
        &game.organizer_email,
        SessionKind::Organizer,
        auth::ORGANIZER_LINK_SECS,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;

    let login_url = auth::login_url(&state, &login, Some(&format!("/jogo/{}", game.id)));
    state
        .enqueue(&compose::welcome(&state.site, &game, &login_url)?)
        .await?;

    let jar = set_flash(
        jar.add(cookie),
        FlashKind::Ok,
        "Email confirmado e sorteio criado. Agora adicione os participantes.",
    );
    Ok((jar, Redirect::to(&format!("/jogo/{}", game.id))).into_response())
}

pub async fn resend_code(
    State(state): State<AppState>,
    Path(id): Path<VerificationId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let v = load_verification(&state, id).await?;
    let now = db::now();
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    if !repo::rate::allow(&mut conn, &format!("verify:{}", v.email), 3, 3600, now).await? {
        let jar = set_flash(
            jar,
            FlashKind::Error,
            "Já enviamos vários códigos para este email na última hora. Confira o spam ou aguarde.",
        );
        return Ok((jar, Redirect::to(&format!("/verificar/{id}"))).into_response());
    }
    let code = VerificationCode::generate();
    repo::verifications::replace_code(
        &mut conn,
        id,
        &code.hash(id),
        now + VERIFICATION_MINUTES * 60,
    )
    .await?;
    drop(conn);
    state
        .enqueue(&compose::verification(
            &state.site,
            &v.email,
            &v.organizer_name,
            &v.name,
            code,
        )?)
        .await?;
    let jar = set_flash(
        jar,
        FlashKind::Ok,
        format!("Novo código enviado para {}.", v.email),
    );
    Ok((jar, Redirect::to(&format!("/verificar/{id}"))).into_response())
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ---------------------------------------------------------------------------------------------
// "Perdi meu link"
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct RecoverQuery {
    pub next: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "recover.html")]
pub struct RecoverPage {
    pub ctx: PageCtx,
    pub email: String,
    pub sent: bool,
    pub error: Option<String>,
    pub wants_game: bool,
}

pub async fn recover_form(
    MaybeSession(session): MaybeSession,
    Query(q): Query<RecoverQuery>,
    jar: CookieJar,
) -> Response {
    let (jar, flash) = take_flash(jar);
    let wants_game = q.next.as_deref().is_some_and(|n| n.starts_with("/jogo/"));
    (
        jar,
        RecoverPage {
            ctx: PageCtx::new(flash, session),
            email: String::new(),
            sent: false,
            error: None,
            wants_game,
        },
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct RecoverForm {
    #[serde(default)]
    pub email: String,
}

pub async fn recover(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    headers: HeaderMap,
    jar: CookieJar,
    Form(form): Form<RecoverForm>,
) -> Result<Response, AppError> {
    let ctx = PageCtx::new(None, session);
    let Ok(email) = form.email.parse::<EmailAddress>() else {
        return Ok((
            jar,
            RecoverPage {
                ctx,
                email: form.email,
                sent: false,
                error: Some("Confira o email digitado.".into()),
                wants_game: false,
            },
        )
            .into_response());
    };
    let now = db::now();
    let ip = client_ip(&headers, state.cfg.trust_proxy);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let allowed = repo::rate::allow(&mut conn, &format!("recover:{email}"), 2, 3600, now).await?
        && repo::rate::allow(&mut conn, &format!("recover_ip:{ip}"), 10, 3600, now).await?;
    let sent_page = RecoverPage {
        ctx,
        email: email.to_string(),
        sent: true,
        error: None,
        wants_game: false,
    };
    if !allowed {
        // Same response as success: don't leak whether the address is known or throttled.
        return Ok((jar, sent_page).into_response());
    }

    let games = repo::games::list_by_organizer(&mut conn, &email).await?;
    let mut organized = Vec::new();
    if !games.is_empty() {
        let secret = auth::issue_login_token(
            &mut conn,
            &email,
            SessionKind::Organizer,
            auth::ORGANIZER_LINK_SECS,
        )
        .await?;
        for g in &games {
            organized.push(LinkItem {
                game_name: g.name.clone(),
                event_date: fmt::date_long(g.event_date),
                url: auth::login_url(&state, &secret, Some(&format!("/jogo/{}", g.id))),
            });
        }
    }

    let mut participations = Vec::new();
    for p in repo::participants::list_by_email(&mut conn, &email).await? {
        let Some(g) = repo::games::get(&mut conn, p.game_id).await? else {
            continue;
        };
        if !g.is_drawn() {
            continue;
        }
        // We only keep hashes, so recovering a link means issuing a new one.
        let secret = Secret::generate();
        repo::participants::set_token_hash(&mut conn, p.id, &secret.hash()).await?;
        participations.push(LinkItem {
            game_name: g.name.clone(),
            event_date: fmt::date_long(g.event_date),
            url: state.url(&format!("/revelar/{}", secret.as_str())),
        });
    }
    drop(conn);

    if !organized.is_empty() || !participations.is_empty() {
        state
            .enqueue(&compose::recover(
                &state.site,
                &email,
                &organized,
                &participations,
            )?)
            .await?;
    } else {
        tracing::info!("recovery requested for unknown email");
    }
    Ok((jar, sent_page).into_response())
}

// ---------------------------------------------------------------------------------------------
// Login via emailed link
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct LoginQuery {
    pub next: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<LoginQuery>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let invalid = || {
        AppError::not_found(
            "Este link de acesso não é válido ou já expirou. Peça um novo em “Perdi meu link”.",
        )
    };
    let secret: Secret = token.parse().map_err(|_| invalid())?;
    let now = db::now();
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let t = repo::auth::get_login_token(&mut conn, &secret.hash(), now)
        .await?
        .ok_or_else(invalid)?;
    if t.kind == SessionKind::Admin {
        if !state.cfg.is_admin(&t.email) {
            return Err(AppError::Forbidden);
        }
        // Admin links are single-use.
        repo::auth::delete_login_tokens_for(&mut conn, &t.email, SessionKind::Admin).await?;
    }
    let cookie = auth::start_session(&mut conn, &t.email, t.kind).await?;
    let default = match t.kind {
        SessionKind::Organizer => "/meus",
        SessionKind::Admin => "/admin",
    };
    let next = auth::safe_next(q.next.as_deref(), default);
    Ok((jar.add(cookie), Redirect::to(&next)).into_response())
}

// ---------------------------------------------------------------------------------------------
// Organizer's games + logout
// ---------------------------------------------------------------------------------------------

#[derive(Template, WebTemplate)]
#[template(path = "my_games.html")]
pub struct MyGamesPage {
    pub ctx: PageCtx,
    pub email: String,
    pub games: Vec<Game>,
}

pub async fn my_games(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let games = repo::games::list_by_organizer(&mut conn, &session.email).await?;
    let (jar, flash) = take_flash(jar);
    let email = session.email.to_string();
    Ok((
        jar,
        MyGamesPage {
            ctx: PageCtx::new(flash, Some(session)),
            email,
            games,
        },
    )
        .into_response())
}

pub async fn logout(
    State(state): State<AppState>,
    MaybeSession(session): MaybeSession,
    jar: CookieJar,
) -> Result<Response, AppError> {
    if let Some(SessionInfo { id_hash, .. }) = session {
        let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
        repo::auth::delete_session(&mut conn, &id_hash).await?;
    }
    let jar = set_flash(
        jar.remove(auth::clear_session_cookie()),
        FlashKind::Info,
        "Você saiu. Até a próxima!",
    );
    Ok((jar, Redirect::to("/")).into_response())
}

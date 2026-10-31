//! HTTP layer: router, shared state and middleware.

pub mod admin;
pub mod assets;
pub mod auth;
pub mod ctx;
pub mod error;
pub mod game;
pub mod home;
pub mod pages;
pub mod reveal;
pub mod security;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::{get, post};
use tokio::sync::Notify;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::config::Config;
use crate::db::SqlitePool;
use crate::mail::Mailer;
use crate::mail::compose::SiteInfo;
use crate::repo::outbox::NewEmail;

pub use error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub cfg: Arc<Config>,
    pub mailer: Mailer,
    pub mail_notify: Arc<Notify>,
    pub limiter: Arc<security::IpLimiter>,
    pub site: Arc<SiteInfo>,
}

impl AppState {
    pub fn new(db: SqlitePool, cfg: Config, mailer: Mailer) -> Self {
        let site = SiteInfo {
            name: "Amigo Oculto Simples".to_owned(),
            url: cfg.base_url.to_string(),
        };
        Self {
            db,
            cfg: Arc::new(cfg),
            mailer,
            mail_notify: Arc::new(Notify::new()),
            limiter: Arc::new(security::IpLimiter::default()),
            site: Arc::new(site),
        }
    }

    /// Queues an email and wakes the worker. Call after the surrounding transaction commits.
    pub async fn enqueue(&self, email: &NewEmail) -> Result<(), AppError> {
        let mut conn = self.db.acquire().await.map_err(anyhow::Error::from)?;
        crate::repo::outbox::enqueue(&mut conn, email, crate::db::now()).await?;
        self.mail_notify.notify_one();
        Ok(())
    }

    pub fn url(&self, path: &str) -> String {
        self.cfg.url(path).to_string()
    }
}

pub fn router(state: AppState) -> Router {
    let trace = TraceLayer::new_for_http()
        .make_span_with(|req: &Request<_>| {
            tracing::info_span!(
                "request",
                method = %req.method(),
                path = %security::redact_path(req.uri().path()),
            )
        })
        .on_request(())
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    Router::new()
        .route("/", get(home::index).post(home::create))
        .route("/criar", post(home::create))
        .route("/verificar/{id}", get(home::verify_form).post(home::verify))
        .route("/verificar/{id}/reenviar", post(home::resend_code))
        .route("/recuperar", get(home::recover_form).post(home::recover))
        .route("/entrar/{token}", get(home::login))
        .route("/meus", get(home::my_games))
        .route("/sair", post(home::logout))
        .route("/jogo/{id}", get(game::show))
        .route("/jogo/{id}/editar", get(game::edit_form).post(game::edit))
        .route("/jogo/{id}/mensagem", post(game::set_message))
        .route("/jogo/{id}/participantes", post(game::add_participant))
        .route(
            "/jogo/{id}/participantes/lote",
            post(game::add_participants_bulk),
        )
        .route(
            "/jogo/{id}/participantes/editar",
            post(game::edit_participant),
        )
        .route(
            "/jogo/{id}/participantes/{pid}/remover",
            post(game::remove_participant),
        )
        .route(
            "/jogo/{id}/participantes/{pid}/reenviar",
            post(game::resend_invite),
        )
        .route("/jogo/{id}/restricoes", post(game::add_exclusion))
        .route(
            "/jogo/{id}/restricoes/remover",
            post(game::remove_exclusion),
        )
        .route("/jogo/{id}/sortear", post(game::draw))
        .route("/jogo/{id}/reenviar-pendentes", post(game::resend_pending))
        .route("/jogo/{id}/excluir", post(game::delete))
        .route("/revelar/{token}", get(reveal::show).post(reveal::reveal))
        .route("/revelar/{token}/dicas", post(reveal::save_wishlist))
        .route("/revelar/{token}/evento.ics", get(reveal::ics))
        .route("/admin", get(admin::index))
        .route("/admin/entrar", post(admin::request_link))
        .route("/admin/jogos/{id}/excluir", post(admin::delete_game))
        .route("/privacidade", get(pages::privacy))
        .route("/termos", get(pages::terms))
        .route("/healthz", get(pages::healthz))
        .route("/static/{*path}", get(assets::serve))
        .fallback(pages::not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .layer(middleware::from_fn(security::headers))
        .layer(RequestBodyLimitLayer::new(64 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(trace)
        .with_state(state)
}

mod db;
mod email;
mod email_templates;
mod matching;
mod models;
mod routes;
mod token;

use anyhow::Context;
use axum::{
    Router,
    routing::{get, patch, post},
};
use email::{EmailConfig, EmailService};
use routes::AppState;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "amigo_oculto_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Get configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/amigo_oculto.db".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;

    tracing::info!("Connecting to database: {}", database_url);

    // Initialize database
    let pool = db::init_db(&database_url).await?;

    // Spawn background task to cleanup expired verifications
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
        loop {
            interval.tick().await;
            match db::cleanup_expired_verifications(&cleanup_pool).await {
                Ok(count) if count > 0 => {
                    tracing::info!("Cleaned up {} expired verification(s)", count);
                }
                Ok(_) => {
                    tracing::debug!("No expired verifications to clean up");
                }
                Err(e) => {
                    tracing::error!("Failed to cleanup expired verifications: {}", e);
                }
            }
        }
    });

    // Initialize email service (already cloneable via internal Arc)
    let email_config = EmailConfig {
        smtp_host: std::env::var("SMTP_HOST")?,
        smtp_port: std::env::var("SMTP_PORT")?.parse()?,
        smtp_username: std::env::var("SMTP_USERNAME")?,
        smtp_password: std::env::var("SMTP_PASSWORD")?,
        from_address: std::env::var("SMTP_FROM")?,
        base_url: std::env::var("BASE_URL")?.parse()?,
    };
    let email_service = EmailService::new(email_config)?;
    email_service.test().await.context("testing connection")?;

    // Create app state (EmailService is cloneable, no Arc needed)
    let state = Arc::new(AppState {
        pool,
        email_service,
    });

    // Build API routes
    let api_routes = Router::new()
        .route("/verifications/request", post(routes::request_verification))
        .route("/verifications/verify", post(routes::verify_code))
        .route("/verifications/resend", post(routes::resend_verification))
        .route("/games", post(routes::create_game))
        .route(
            "/games/{game_id}/participants",
            post(routes::add_participant),
        )
        .route("/games/{game_id}/draw", post(routes::draw_game))
        .route("/games/{game_id}/resend-all", post(routes::resend_all_emails))
        .route(
            "/games/{game_id}/participants/{participant_id}/resend",
            post(routes::resend_participant_email),
        )
        .route(
            "/games/{game_id}/participants/{participant_id}",
            patch(routes::update_participant),
        )
        .route("/games/{game_id}", get(routes::get_game_status).delete(routes::delete_game))
        .route("/reveal/{view_token}", get(routes::reveal_match))
        .with_state(state);

    // Setup CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build main app
    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("../frontend/build").not_found_service(
            ServeDir::new("../frontend/build").append_index_html_on_directories(true),
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("🚀 Server listening on {}", addr);
    tracing::info!("📝 API available at http://localhost:{}/api", port);

    axum::serve(listener, app).await?;

    Ok(())
}
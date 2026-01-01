mod db;
mod email;
mod email_templates;
mod matching;
mod models;
mod routes;
mod server;
mod site_admin_auth;
mod staging_auth;
mod token;

use crate::{db::Database, server::Server};
use anyhow::Context;
use email::EmailService;
use tokio_util::sync::CancellationToken;
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

    let db = Database::from_env().await?;

    // Initialize site admin password if not already set
    db.init_site_admin_password().await.context("initializing site admin password")?;

    let cancel = CancellationToken::new();
    let server = Server::new(&db, cancel.clone())?;
    let email_service = EmailService::from_env()?;

    email_service.test().await.context("testing connection")?;

    let app = routes::make(db, email_service);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("🚀 Server listening on {}", addr);
    tracing::info!("📝 App available at http://localhost:{}/", port);

    // Run the HTTP server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel))
        .await?;

    // Shutdown background tasks
    server.shutdown().await;

    Ok(())
}

async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, starting graceful shutdown");
    cancel.cancel();
}

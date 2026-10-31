use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use amigo_oculto::config::MailConfig;
use amigo_oculto::mail::{Mailer, worker};
use amigo_oculto::{AppState, Config, cleanup, db, router};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cfg = Config::from_env()?;
    let pool = db::connect(&cfg.database_url).await?;
    let mailer = Mailer::from_config(&cfg.mail)?;
    match &cfg.mail {
        MailConfig::Smtp(s) => {
            tracing::info!(host = %s.host, port = s.port, "mail transport: smtp")
        }
        MailConfig::Log { .. } => {
            tracing::warn!("mail transport: log (emails are printed, not sent)")
        }
    }
    if let Err(e) = mailer.test().await {
        // Keep serving: queued mail is retried by the worker, and a dead SMTP relay should not
        // take the organizer pages down with it.
        tracing::error!(error = %e, "SMTP self-test failed; emails will be retried in the background");
    }

    let port = cfg.port;
    let retention_days = cfg.game_retention_days;
    let state = AppState::new(pool.clone(), cfg, mailer.clone());
    let cancel = CancellationToken::new();

    let worker_task = tokio::spawn(worker::run(
        pool.clone(),
        mailer,
        Arc::clone(&state.mail_notify),
        cancel.clone(),
    ));
    let cleanup_task = tokio::spawn(cleanup::run(pool.clone(), retention_days, cancel.clone()));

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, version = env!("CARGO_PKG_VERSION"), "server listening");

    let shutdown = cancel.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            wait_for_signal().await;
            tracing::info!("shutdown signal received");
            shutdown.cancel();
        })
        .await
        .context("serving")?;

    cancel.cancel();
    let _ = tokio::join!(worker_task, cleanup_task);
    pool.close().await;
    tracing::info!("bye");
    Ok(())
}

async fn wait_for_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

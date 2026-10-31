//! Process configuration, read once from the environment.

use std::collections::HashSet;
use std::env;

use anyhow::{Context, Result, bail};
use url::Url;

use crate::email_addr::EmailAddress;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    /// Public origin, e.g. `https://amigoocultosimples.app` (no trailing path).
    pub base_url: Url,
    pub database_url: String,
    pub mail: MailConfig,
    /// Emails allowed to log in to `/admin`.
    pub admin_emails: HashSet<EmailAddress>,
    /// Trust `X-Forwarded-For` for the client IP (true behind Railway/Cloudflare).
    pub trust_proxy: bool,
    /// Days after the event date before a game and all its data are deleted.
    pub game_retention_days: u32,
}

#[derive(Debug, Clone)]
pub enum MailConfig {
    /// Deliver over SMTP (production).
    Smtp(SmtpConfig),
    /// Only log outgoing mail (local development and container smoke tests).
    Log {
        from: EmailAddress,
        from_name: String,
    },
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: EmailAddress,
    pub from_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let port = env_or("PORT", "3000")
            .parse()
            .context("PORT must be a number")?;

        let base_url: Url = env::var("BASE_URL")
            .context("BASE_URL is required (e.g. https://amigoocultosimples.app)")?
            .parse()
            .context("BASE_URL must be an absolute URL")?;
        if base_url.cannot_be_a_base() || base_url.host_str().is_none() {
            bail!("BASE_URL must be an absolute http(s) URL");
        }
        let mut base_url = base_url;
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);

        // v2 uses a new file name so a volume that still holds the v1 database starts fresh.
        let database_url = env_or("DATABASE_URL", "sqlite:///app/data/amigo_oculto_v2.db");

        let from: EmailAddress = env_or("SMTP_FROM", "naoresponda@amigoocultosimples.app")
            .parse()
            .context("SMTP_FROM must be a valid email address")?;
        let from_name = env_or("SMTP_FROM_NAME", "Amigo Oculto Simples");
        let transport = env_or(
            "MAIL_TRANSPORT",
            if env::var("SMTP_HOST").is_ok() {
                "smtp"
            } else {
                "log"
            },
        );
        let mail = match transport.as_str() {
            "smtp" => MailConfig::Smtp(SmtpConfig {
                host: env::var("SMTP_HOST").context("SMTP_HOST is required")?,
                port: env_or("SMTP_PORT", "587")
                    .parse()
                    .context("SMTP_PORT must be a number")?,
                username: env::var("SMTP_USERNAME").context("SMTP_USERNAME is required")?,
                password: env::var("SMTP_PASSWORD").context("SMTP_PASSWORD is required")?,
                from,
                from_name,
            }),
            "log" => MailConfig::Log { from, from_name },
            other => bail!("MAIL_TRANSPORT must be 'smtp' or 'log', got {other:?}"),
        };

        let admin_emails = env::var("ADMIN_EMAILS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<EmailAddress>()
                    .with_context(|| format!("ADMIN_EMAILS entry {s:?}"))
            })
            .collect::<Result<HashSet<_>>>()?;

        let trust_proxy = env_or("TRUST_PROXY", "true") == "true";
        let game_retention_days = env_or("GAME_RETENTION_DAYS", "90")
            .parse()
            .context("GAME_RETENTION_DAYS must be a number")?;

        Ok(Self {
            port,
            base_url,
            database_url,
            mail,
            admin_emails,
            trust_proxy,
            game_retention_days,
        })
    }

    /// Absolute URL for a site path such as `/jogo/123`.
    pub fn url(&self, path: &str) -> Url {
        self.base_url
            .join(path.trim_start_matches('/'))
            .expect("valid path")
    }

    pub fn origin(&self) -> String {
        self.base_url.origin().ascii_serialization()
    }

    pub fn is_https(&self) -> bool {
        self.base_url.scheme() == "https"
    }

    pub fn is_admin(&self, email: &EmailAddress) -> bool {
        self.admin_emails.contains(email)
    }

    pub fn from_address(&self) -> (&EmailAddress, &str) {
        match &self.mail {
            MailConfig::Smtp(s) => (&s.from, &s.from_name),
            MailConfig::Log { from, from_name } => (from, from_name),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

use crate::email_templates::{html, plain};
use crate::token::{AdminToken, EmailAddress, GameId, ViewToken};
use anyhow::Result;
use chrono::{Locale, NaiveDate};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::sync::Arc;
use url::Url;

type SmtpTransport = AsyncSmtpTransport<Tokio1Executor>;

pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_address: String,
    pub base_url: Url,
}

impl EmailConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            smtp_host: std::env::var("SMTP_HOST")?,
            smtp_port: std::env::var("SMTP_PORT")?.parse()?,
            smtp_username: std::env::var("SMTP_USERNAME")?,
            smtp_password: std::env::var("SMTP_PASSWORD")?,
            from_address: std::env::var("SMTP_FROM")?,
            base_url: std::env::var("BASE_URL")?.parse()?,
        })
    }
}

#[derive(Clone)]
pub struct EmailService {
    inner: Arc<EmailServiceInner>,
}

struct EmailServiceInner {
    mailer: SmtpTransport,
    from_address: Mailbox,
    base_url: Url,
}

impl EmailService {
    pub fn from_env() -> Result<Self> {
        let config = EmailConfig::from_env()?;
        Self::new(config)
    }

    pub fn new(config: EmailConfig) -> Result<Self> {
        let creds = Credentials::new(config.smtp_username, config.smtp_password);

        let mailer =
            // Port 587 requires STARTTLS (upgrade from plain to TLS)
            SmtpTransport::starttls_relay(&config.smtp_host)?
                .port(config.smtp_port)
                .credentials(creds)
                .build();

        let email_address = config.from_address.parse()?;
        let from_address = Mailbox::new(Some("Amigo Oculto".to_string()), email_address);

        Ok(Self {
            inner: EmailServiceInner {
                mailer,
                from_address,
                base_url: config.base_url,
            }
            .into(),
        })
    }

    pub async fn test(&self) -> Result<()> {
        tracing::info!("testing SMTP connection...");
        self.inner.mailer.test_connection().await.map_err(|e| {
            tracing::error!("SMTP connection test failed: {}", e);
            anyhow::anyhow!("failed to connect to SMTP server: {}. Please verify your SMTP settings (host, port, username, password) and network connectivity.", e)
        })?;
        tracing::info!("SMTP connection test successful");
        Ok(())
    }

    fn reveal_url(&self, view_token: &ViewToken) -> Url {
        self.inner
            .base_url
            .join(&format!("revelar/{}", view_token))
            .unwrap()
    }

    pub async fn send_participant_notification(
        &self,
        participant_name: &str,
        participant_email: &EmailAddress,
        game_name: &str,
        event_date: NaiveDate,
        view_token: &ViewToken,
    ) -> Result<()> {
        let reveal_url = self.reveal_url(view_token);
        let formatted_date = format_brazilian_date(event_date);

        // Generate HTML using Maud template (XSS-safe)
        let html_body =
            html::participant_email(participant_name, game_name, &formatted_date, &reveal_url)
                .into_string();

        // Generate plain-text
        let plain_body =
            plain::participant_email(participant_name, game_name, &formatted_date, &reveal_url);

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(participant_email.to_mailbox())
            .subject(format!("🎁 {}", game_name))
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(plain_body),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )?;

        self.inner.mailer.send(email).await?;
        Ok(())
    }

    fn admin_url(&self, game_id: GameId, admin_token: &AdminToken) -> Url {
        let mut url = self.inner.base_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("jogo")
            .push(&game_id.to_string());
        url.query_pairs_mut()
            .append_pair("admin_token", admin_token.as_str());
        url
    }

    pub async fn send_organizer_confirmation(
        &self,
        organizer_email: &EmailAddress,
        game_name: &str,
        event_date: NaiveDate,
        game_id: GameId,
        admin_token: &AdminToken,
        participant_count: usize,
    ) -> Result<()> {
        let admin_url = self.admin_url(game_id, admin_token);
        let formatted_date = format_brazilian_date(event_date);

        // Generate HTML using Maud template (XSS-safe)
        let html_body =
            html::organizer_email(game_name, &formatted_date, participant_count, &admin_url)
                .into_string();

        // Generate plain-text
        let plain_body =
            plain::organizer_email(game_name, &formatted_date, participant_count, &admin_url);

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(organizer_email.to_mailbox())
            .subject(format!("✅ Sorteio Realizado: {}", game_name))
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(plain_body),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )?;

        self.inner.mailer.send(email).await?;
        Ok(())
    }

    pub async fn send_verification_code(
        &self,
        recipient_email: &EmailAddress,
        game_name: &str,
        verification_code: &str,
    ) -> Result<()> {
        // Generate HTML using Maud template (XSS-safe)
        let html_body = html::verification_email(game_name, verification_code).into_string();

        // Generate plain-text
        let plain_body = plain::verification_email(game_name, verification_code);

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(recipient_email.to_mailbox())
            .subject("🔐 Código de Verificação")
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(plain_body),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )?;

        self.inner.mailer.send(email).await?;
        Ok(())
    }

    pub async fn send_admin_welcome(
        &self,
        organizer_email: &EmailAddress,
        game_name: &str,
        event_date: NaiveDate,
        game_id: GameId,
        admin_token: &AdminToken,
    ) -> Result<()> {
        let admin_url = self.admin_url(game_id, admin_token);
        let formatted_date = format_brazilian_date(event_date);

        // Generate HTML using Maud template (XSS-safe)
        let html_body =
            html::admin_welcome_email(game_name, &formatted_date, &admin_url).into_string();

        // Generate plain-text
        let plain_body = plain::admin_welcome_email(game_name, &formatted_date, &admin_url);

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(organizer_email.to_mailbox())
            .subject(format!("🎉 Jogo Criado: {}", game_name))
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(plain_body),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body),
                    ),
            )?;

        self.inner.mailer.send(email).await?;
        Ok(())
    }
}

/// Formats a date in Brazilian Portuguese format (e.g., "25 de dezembro de 2024")
fn format_brazilian_date(date: NaiveDate) -> String {
    date.format_localized("%e de %B de %Y", Locale::pt_BR)
        .to_string()
}

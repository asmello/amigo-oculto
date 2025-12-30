use anyhow::Result;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::sync::Arc;
use url::Url;
use crate::email_templates::{html, plain};
use crate::token::SecureToken;

type SmtpTransport = AsyncSmtpTransport<Tokio1Executor>;

pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_address: String,
    pub base_url: Url,
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
    pub fn new(config: EmailConfig) -> Result<Self> {
        let creds = Credentials::new(config.smtp_username, config.smtp_password);

        let mailer =  
            // Port 587 requires STARTTLS (upgrade from plain to TLS)
            SmtpTransport::starttls_relay(&config.smtp_host)?
                .port(config.smtp_port)
                .credentials(creds)
                .build() ;

        Ok(Self {
            inner: EmailServiceInner {
                mailer,
                from_address: config.from_address.parse()?,
                base_url: config.base_url,
            }
            .into(),
        })
    }

    pub async fn test(&self) -> Result<()> {
        tracing::info!("Testing SMTP connection...");
        self.inner.mailer.test_connection().await
            .map_err(|e| {
                tracing::error!("SMTP connection test failed: {}", e);
                anyhow::anyhow!("Failed to connect to SMTP server: {}. Please verify your SMTP settings (host, port, username, password) and network connectivity.", e)
            })?;
        tracing::info!("✓ SMTP connection test successful!");
        Ok(())
    }

    fn reveal_url(&self, view_token: &str) -> Url {
        self.inner
            .base_url
            .join(&format!("revelar/{view_token}"))
            .unwrap()
    }

    pub async fn send_participant_notification(
        &self,
        participant_name: &str,
        participant_email: &str,
        game_name: &str,
        event_date: &str,
        view_token: &SecureToken,
    ) -> Result<()> {
        let reveal_url = self.reveal_url(view_token.as_str());

        // Generate HTML using Maud template (XSS-safe)
        let html_body = html::participant_email(
            participant_name,
            game_name,
            event_date,
            &reveal_url,
        ).into_string();

        // Generate plain-text
        let plain_body = plain::participant_email(
            participant_name,
            game_name,
            event_date,
            &reveal_url,
        );

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(participant_email.parse()?)
            .subject(format!("🎁 Amigo Oculto: {}", game_name))
            .header(ContentType::TEXT_HTML)
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

    fn admin_url(&self, game_id: &str, admin_token: &str) -> Url {
        let mut url = self.inner.base_url.clone();
        url.path_segments_mut().unwrap().push("jogo").push(game_id);
        url.query_pairs_mut()
            .append_pair("admin_token", admin_token);
        url
    }

    pub async fn send_organizer_confirmation(
        &self,
        organizer_email: &str,
        game_name: &str,
        event_date: &str,
        game_id: &str,
        admin_token: &SecureToken,
        participant_count: usize,
    ) -> Result<()> {
        let admin_url = self.admin_url(game_id, admin_token.as_str());

        // Generate HTML using Maud template (XSS-safe)
        let html_body = html::organizer_email(
            game_name,
            event_date,
            participant_count,
            &admin_url,
        ).into_string();

        // Generate plain-text
        let plain_body = plain::organizer_email(
            game_name,
            event_date,
            participant_count,
            &admin_url,
        );

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(organizer_email.parse()?)
            .subject(format!("✅ Sorteio Realizado: {}", game_name))
            .header(ContentType::TEXT_HTML)
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
        recipient_email: &str,
        game_name: &str,
        verification_code: &str,
    ) -> Result<()> {
        // Generate HTML using Maud template (XSS-safe)
        let html_body = html::verification_email(
            game_name,
            verification_code,
        ).into_string();

        // Generate plain-text
        let plain_body = plain::verification_email(
            game_name,
            verification_code,
        );

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(recipient_email.parse()?)
            .subject("🔐 Código de Verificação - Amigo Oculto")
            .header(ContentType::TEXT_HTML)
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
        organizer_email: &str,
        game_name: &str,
        event_date: &str,
        game_id: &str,
        admin_token: &SecureToken,
    ) -> Result<()> {
        let admin_url = self.admin_url(game_id, admin_token.as_str());

        // Generate HTML using Maud template (XSS-safe)
        let html_body = html::admin_welcome_email(
            game_name,
            event_date,
            &admin_url,
        ).into_string();

        // Generate plain-text
        let plain_body = plain::admin_welcome_email(
            game_name,
            event_date,
            &admin_url,
        );

        let email = Message::builder()
            .from(self.inner.from_address.clone())
            .to(organizer_email.parse()?)
            .subject(format!("🎉 Jogo Criado: {}", game_name))
            .header(ContentType::TEXT_HTML)
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
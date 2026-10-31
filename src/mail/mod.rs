//! Outgoing email: transports, composition and the outbox worker.

pub mod compose;
pub mod ics;
pub mod worker;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use lettre::message::{Mailbox, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{MailConfig, SmtpConfig};
use crate::models::OutboxEmail;

/// Why a send failed, which decides whether the worker retries.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// The server rejected the message for good (e.g. unknown recipient). Do not retry.
    #[error("rejeitado pelo servidor: {0}")]
    Permanent(String),
    /// Connection or temporary server trouble. Retry later.
    #[error("falha temporária: {0}")]
    Transient(String),
}

/// A copy of a message captured by [`Mailer::Memory`] (tests, local development).
#[derive(Debug, Clone)]
pub struct CapturedEmail {
    pub to: String,
    pub reply_to: Option<String>,
    pub subject: String,
    pub text_body: String,
    pub html_body: String,
}

#[derive(Clone)]
pub enum Mailer {
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
    },
    /// Logs each message instead of sending it.
    Log { from: Mailbox },
    /// Captures messages for inspection.
    Memory(Arc<Mutex<Vec<CapturedEmail>>>),
    /// Fails every send (tests of the retry logic).
    Failing { permanent: bool },
}

impl Mailer {
    pub fn from_config(cfg: &MailConfig) -> Result<Self> {
        match cfg {
            MailConfig::Smtp(s) => Self::smtp(s),
            MailConfig::Log { from, from_name } => Ok(Self::Log {
                from: from.to_mailbox(Some(from_name)),
            }),
        }
    }

    pub fn memory() -> Self {
        Self::Memory(Arc::new(Mutex::new(Vec::new())))
    }

    fn smtp(s: &SmtpConfig) -> Result<Self> {
        let creds = Credentials::new(s.username.clone(), s.password.clone());
        // 465/8465 speak TLS from the first byte; everything else (587, 2525, 8025) upgrades
        // with STARTTLS.
        let builder = if matches!(s.port, 465 | 8465) {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host)
        }
        .with_context(|| format!("configuring SMTP relay {}", s.host))?;
        let transport = builder
            .port(s.port)
            .credentials(creds)
            .timeout(Some(Duration::from_secs(30)))
            .build();
        Ok(Self::Smtp {
            transport,
            from: s.from.to_mailbox(Some(&s.from_name)),
        })
    }

    /// Verifies connectivity and credentials (used at startup for SMTP only).
    pub async fn test(&self) -> Result<()> {
        if let Self::Smtp { transport, .. } = self {
            let ok = tokio::time::timeout(Duration::from_secs(30), transport.test_connection())
                .await
                .context("SMTP connection test timed out")?
                .context("SMTP connection test failed")?;
            anyhow::ensure!(ok, "SMTP server did not accept the connection");
        }
        Ok(())
    }

    pub fn captured(&self) -> Vec<CapturedEmail> {
        match self {
            Self::Memory(store) => store.lock().expect("mailer store poisoned").clone(),
            _ => Vec::new(),
        }
    }

    pub async fn send(&self, email: &OutboxEmail) -> Result<(), SendError> {
        match self {
            Self::Smtp { transport, from } => {
                let message = build_message(from.clone(), email)?;
                transport
                    .send(message)
                    .await
                    .map(|_| ())
                    .map_err(classify_smtp_error)
            }
            Self::Log { from } => {
                // Build it anyway so template/address bugs surface in development.
                build_message(from.clone(), email)?;
                tracing::info!(
                    to = %email.to_email,
                    subject = %email.subject,
                    body = %email.text_body,
                    "email (log transport)"
                );
                Ok(())
            }
            Self::Memory(store) => {
                store
                    .lock()
                    .expect("mailer store poisoned")
                    .push(CapturedEmail {
                        to: email.to_email.to_string(),
                        reply_to: email.reply_to.as_ref().map(ToString::to_string),
                        subject: email.subject.clone(),
                        text_body: email.text_body.clone(),
                        html_body: email.html_body.clone(),
                    });
                Ok(())
            }
            Self::Failing { permanent: true } => {
                Err(SendError::Permanent("recusado (teste)".into()))
            }
            Self::Failing { permanent: false } => {
                Err(SendError::Transient("indisponível (teste)".into()))
            }
        }
    }
}

fn build_message(from: Mailbox, email: &OutboxEmail) -> Result<Message, SendError> {
    let mut builder = Message::builder()
        .from(from)
        .to(email.to_email.to_mailbox(email.to_name.as_deref()))
        .subject(&email.subject);
    if let Some(reply_to) = &email.reply_to {
        builder = builder.reply_to(reply_to.to_mailbox(None));
    }
    builder
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(email.text_body.clone()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(email.html_body.clone()),
                ),
        )
        .map_err(|e| SendError::Permanent(format!("montando a mensagem: {e}")))
}

fn classify_smtp_error(e: lettre::transport::smtp::Error) -> SendError {
    if e.is_permanent() {
        SendError::Permanent(e.to_string())
    } else {
        SendError::Transient(e.to_string())
    }
}

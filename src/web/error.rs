//! Application error type rendered as a friendly HTML page.

use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use super::ctx::PageCtx;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("acesso negado")]
    Forbidden,
    #[error("muitas tentativas; aguarde um pouco")]
    TooManyRequests,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.into())
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        Self::Internal(e.into())
    }
}

impl AppError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorPage {
    pub ctx: PageCtx,
    pub status: u16,
    pub title: String,
    pub detail: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let (title, detail) = match &self {
            Self::NotFound(m) => ("Não encontramos isso".to_owned(), m.clone()),
            Self::BadRequest(m) => ("Algo não bateu".to_owned(), m.clone()),
            Self::Forbidden => (
                "Acesso negado".to_owned(),
                "Você não tem permissão para fazer isso.".to_owned(),
            ),
            Self::TooManyRequests => (
                "Calma aí".to_owned(),
                "Muitas tentativas em pouco tempo. Aguarde alguns minutos e tente de novo."
                    .to_owned(),
            ),
            Self::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (
                    "Deu algo errado por aqui".to_owned(),
                    "Já registramos o problema. Tente de novo em instantes.".to_owned(),
                )
            }
        };
        let page = ErrorPage {
            ctx: PageCtx::bare(),
            status: status.as_u16(),
            title,
            detail,
        };
        match page.render() {
            Ok(html) => (status, Html(html)).into_response(),
            Err(e) => {
                tracing::error!(error = %e, "rendering error page");
                (status, "erro").into_response()
            }
        }
    }
}

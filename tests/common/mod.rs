//! Shared harness: the real router on an in-memory database with a capturing mailer.

#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::Arc;

use amigo_oculto::config::{Config, MailConfig};
use amigo_oculto::mail::worker;
use amigo_oculto::mail::{CapturedEmail, Mailer};
use amigo_oculto::{AppState, db, router};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use tower::ServiceExt;

pub const ORIGIN: &str = "http://localhost:3000";

pub struct App {
    pub router: Router,
    pub state: AppState,
    pub mailer: Mailer,
    /// Cookies carried between requests (name → value), like a browser would.
    pub jar: Vec<(String, String)>,
}

pub async fn app() -> App {
    app_with(|_| {}).await
}

pub async fn app_with(tweak: impl FnOnce(&mut Config)) -> App {
    let pool = db::connect_in_memory().await.expect("in-memory db");
    let mut cfg = Config {
        port: 3000,
        base_url: ORIGIN.parse().unwrap(),
        database_url: "sqlite::memory:".into(),
        mail: MailConfig::Log {
            from: "naoresponda@exemplo.app".parse().unwrap(),
            from_name: "Teste".into(),
        },
        admin_emails: HashSet::from(["admin@exemplo.com".parse().unwrap()]),
        trust_proxy: true,
        game_retention_days: 90,
    };
    tweak(&mut cfg);
    let mailer = Mailer::memory();
    let state = AppState::new(pool, cfg, mailer.clone());
    App {
        router: router(state.clone()),
        state,
        mailer,
        jar: Vec::new(),
    }
}

impl App {
    fn cookie_header(&self) -> Option<String> {
        if self.jar.is_empty() {
            return None;
        }
        Some(
            self.jar
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    fn absorb_cookies(&mut self, res: &Response) {
        for value in res.headers().get_all(header::SET_COOKIE) {
            let raw = value.to_str().unwrap();
            let pair = raw.split(';').next().unwrap();
            let (name, val) = pair.split_once('=').unwrap();
            let expired = raw.contains("Max-Age=0") || raw.contains("expires=Thu, 01 Jan 1970");
            self.jar.retain(|(k, _)| k != name);
            if !expired && !val.is_empty() {
                self.jar.push((name.to_owned(), val.to_owned()));
            }
        }
    }

    pub async fn get(&mut self, path: &str) -> Res {
        let mut req = Request::builder().method("GET").uri(path);
        if let Some(c) = self.cookie_header() {
            req = req.header(header::COOKIE, c);
        }
        self.send(req.body(Body::empty()).unwrap()).await
    }

    /// Same-origin form POST, as a browser would send it (with `Origin` + `Sec-Fetch-Site`).
    pub async fn post(&mut self, path: &str, form: &[(&str, &str)]) -> Res {
        let body: String = form
            .iter()
            .map(|(k, v)| format!("{}={}", urlenc(k), urlenc(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut req = Request::builder()
            .method("POST")
            .uri(path)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, ORIGIN)
            .header("sec-fetch-site", "same-origin")
            .header("x-forwarded-for", "203.0.113.7");
        if let Some(c) = self.cookie_header() {
            req = req.header(header::COOKIE, c);
        }
        self.send(req.body(Body::from(body)).unwrap()).await
    }

    pub async fn post_raw(&mut self, req: Request<Body>) -> Res {
        self.send(req).await
    }

    async fn send(&mut self, req: Request<Body>) -> Res {
        let res = self.router.clone().oneshot(req).await.unwrap();
        self.absorb_cookies(&res);
        let status = res.status();
        let location = res
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_owned());
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        Res {
            status,
            location,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    /// Delivers everything queued in the outbox through the capturing mailer.
    pub async fn flush_mail(&self) {
        worker::process_due(&self.state.db, &self.mailer)
            .await
            .expect("outbox");
    }

    /// Flushes the outbox and returns the emails captured since the last call.
    pub async fn take_mail(&self) -> Vec<CapturedEmail> {
        self.flush_mail().await;
        let all = self.mailer.captured();
        if let Mailer::Memory(store) = &self.mailer {
            store.lock().unwrap().clear();
        }
        all
    }

    pub fn clear_cookies(&mut self) {
        self.jar.clear();
    }
}

pub struct Res {
    pub status: StatusCode,
    pub location: Option<String>,
    pub headers: axum::http::HeaderMap,
    pub body: String,
}

impl Res {
    pub fn assert_redirect(&self, to: &str) -> &Self {
        assert!(
            self.status.is_redirection(),
            "expected redirect, got {} with body {}",
            self.status,
            self.body
        );
        assert_eq!(
            self.location.as_deref(),
            Some(to),
            "unexpected redirect target"
        );
        self
    }

    pub fn assert_redirect_starts(&self, prefix: &str) -> String {
        assert!(
            self.status.is_redirection(),
            "expected redirect, got {} with body {}",
            self.status,
            self.body
        );
        let loc = self.location.clone().unwrap();
        assert!(
            loc.starts_with(prefix),
            "redirect {loc} does not start with {prefix}"
        );
        loc
    }

    pub fn assert_ok(&self) -> &Self {
        assert_eq!(self.status, StatusCode::OK, "body: {}", self.body);
        self
    }

    pub fn assert_contains(&self, needle: &str) -> &Self {
        assert!(
            self.body.contains(needle),
            "body does not contain {needle:?}:\n{}",
            self.body
        );
        self
    }

    pub fn assert_not_contains(&self, needle: &str) -> &Self {
        assert!(
            !self.body.contains(needle),
            "body unexpectedly contains {needle:?}"
        );
        self
    }
}

pub fn urlenc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Pulls the first absolute link containing `path_prefix` out of an email body.
pub fn link_in(email: &CapturedEmail, path_prefix: &str) -> String {
    let needle = format!("{ORIGIN}{path_prefix}");
    let start = email
        .text_body
        .find(&needle)
        .unwrap_or_else(|| panic!("no link {needle} in:\n{}", email.text_body));
    let rest = &email.text_body[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '<')
        .unwrap_or(rest.len());
    rest[..end].to_owned()
}

/// Local path (with query) of an absolute link.
pub fn path_of(url: &str) -> String {
    url.strip_prefix(ORIGIN).unwrap().to_owned()
}

/// 6-digit code from a verification email.
pub fn code_in(email: &CapturedEmail) -> String {
    email
        .text_body
        .split_whitespace()
        .find(|w| w.len() == 6 && w.chars().all(|c| c.is_ascii_digit()))
        .expect("code in email")
        .to_owned()
}

/// Creates and verifies a game, leaving the organizer logged in. Returns the game id.
pub async fn create_game(app: &mut App, name: &str, organizer_email: &str) -> String {
    let res = app
        .post(
            "/criar",
            &[
                ("name", name),
                ("event_date", "2027-12-24"),
                ("budget", "80"),
                ("organizer_name", "Ana Mello"),
                ("organizer_email", organizer_email),
            ],
        )
        .await;
    let verify_path = res.assert_redirect_starts("/verificar/");
    let mail = app.take_mail().await;
    let verification = mail
        .iter()
        .find(|m| m.subject.contains("código"))
        .expect("verification email");
    assert_eq!(verification.to, organizer_email);
    let code = code_in(verification);
    let res = app.post(&verify_path, &[("code", &code)]).await;
    let game_path = res.assert_redirect_starts("/jogo/");
    game_path.trim_start_matches("/jogo/").to_owned()
}

pub async fn add_participant(app: &mut App, game_id: &str, name: &str, email: &str) {
    app.post(
        &format!("/jogo/{game_id}/participantes"),
        &[("name", name), ("email", email)],
    )
    .await
    .assert_redirect(&format!("/jogo/{game_id}"));
}

pub fn shared(state: &AppState) -> Arc<amigo_oculto::mail::compose::SiteInfo> {
    state.site.clone()
}

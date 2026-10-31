//! Builds the emails we send. Pure functions: they render templates into a [`NewEmail`];
//! persisting and sending happen elsewhere.

use anyhow::{Context, Result};
use askama::Template;

use crate::email_addr::EmailAddress;
use crate::fmt;
use crate::ids::VerificationCode;
use crate::models::{EmailKind, Game, Participant};
use crate::repo::outbox::NewEmail;

pub const VERIFICATION_MINUTES: i64 = 15;
pub const ADMIN_LINK_MINUTES: i64 = 15;

/// Everything a template may need about the site itself.
#[derive(Clone)]
pub struct SiteInfo {
    pub name: String,
    pub url: String,
}

fn render<T: Template>(t: &T) -> Result<String> {
    t.render().context("rendering email template")
}

// ---------------------------------------------------------------------------------------------
// Verification code (game creation)
// ---------------------------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/verification.html")]
struct VerificationHtml<'a> {
    site: &'a SiteInfo,
    code: String,
    game_name: &'a str,
    organizer_name: &'a str,
    minutes: i64,
}

#[derive(Template)]
#[template(path = "email/verification.txt")]
struct VerificationText<'a> {
    site: &'a SiteInfo,
    code: String,
    game_name: &'a str,
    organizer_name: &'a str,
    minutes: i64,
}

pub fn verification(
    site: &SiteInfo,
    to: &EmailAddress,
    organizer_name: &str,
    game_name: &str,
    code: VerificationCode,
) -> Result<NewEmail> {
    let code = code.to_string();
    let html = VerificationHtml {
        site,
        code: code.clone(),
        game_name,
        organizer_name,
        minutes: VERIFICATION_MINUTES,
    };
    let text = VerificationText {
        site,
        code: code.clone(),
        game_name,
        organizer_name,
        minutes: VERIFICATION_MINUTES,
    };
    Ok(NewEmail {
        game_id: None,
        participant_id: None,
        kind: EmailKind::Verification,
        draw_round: None,
        to_email: to.clone(),
        to_name: Some(organizer_name.to_owned()),
        reply_to: None,
        subject: format!("{code} é o seu código · {}", site.name),
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

// ---------------------------------------------------------------------------------------------
// Welcome (organizer, right after creating the game)
// ---------------------------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/welcome.html")]
struct WelcomeHtml<'a> {
    site: &'a SiteInfo,
    organizer_name: &'a str,
    game_name: &'a str,
    event_date: String,
    login_url: &'a str,
}

#[derive(Template)]
#[template(path = "email/welcome.txt")]
struct WelcomeText<'a> {
    site: &'a SiteInfo,
    organizer_name: &'a str,
    game_name: &'a str,
    event_date: String,
    login_url: &'a str,
}

pub fn welcome(site: &SiteInfo, game: &Game, login_url: &str) -> Result<NewEmail> {
    let event_date = fmt::date_long(game.event_date);
    let html = WelcomeHtml {
        site,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        event_date: event_date.clone(),
        login_url,
    };
    let text = WelcomeText {
        site,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        event_date,
        login_url,
    };
    Ok(NewEmail {
        game_id: Some(game.id),
        participant_id: None,
        kind: EmailKind::Welcome,
        draw_round: None,
        to_email: game.organizer_email.clone(),
        to_name: Some(game.organizer_name.clone()),
        reply_to: None,
        subject: format!("Seu sorteio “{}” foi criado", game.name),
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

// ---------------------------------------------------------------------------------------------
// Invitation (participant, after a draw or on resend)
// ---------------------------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/invite.html")]
struct InviteHtml<'a> {
    site: &'a SiteInfo,
    participant_name: &'a str,
    organizer_name: &'a str,
    game_name: &'a str,
    event_date: String,
    budget: Option<String>,
    message: Option<&'a str>,
    reveal_url: &'a str,
    redraw: bool,
}

#[derive(Template)]
#[template(path = "email/invite.txt")]
struct InviteText<'a> {
    site: &'a SiteInfo,
    participant_name: &'a str,
    organizer_name: &'a str,
    game_name: &'a str,
    event_date: String,
    budget: Option<String>,
    message: Option<&'a str>,
    reveal_url: &'a str,
    redraw: bool,
}

pub fn invite(
    site: &SiteInfo,
    game: &Game,
    p: &Participant,
    reveal_url: &str,
    redraw: bool,
) -> Result<NewEmail> {
    let event_date = fmt::date_long(game.event_date);
    let budget = game.budget_cents.map(fmt::money);
    let html = InviteHtml {
        site,
        participant_name: &p.name,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        event_date: event_date.clone(),
        budget: budget.clone(),
        message: game.message.as_deref(),
        reveal_url,
        redraw,
    };
    let text = InviteText {
        site,
        participant_name: &p.name,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        event_date,
        budget,
        message: game.message.as_deref(),
        reveal_url,
        redraw,
    };
    let subject = if redraw {
        format!("Novo sorteio do amigo oculto “{}”", game.name)
    } else {
        format!(
            "{} convidou você para o amigo oculto “{}”",
            fmt::first_name(&game.organizer_name),
            game.name
        )
    };
    Ok(NewEmail {
        game_id: Some(game.id),
        participant_id: Some(p.id),
        kind: EmailKind::Invite,
        draw_round: Some(game.draw_round),
        to_email: p.email.clone(),
        to_name: Some(p.name.clone()),
        reply_to: Some(game.organizer_email.clone()),
        subject,
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

// ---------------------------------------------------------------------------------------------
// Draw confirmation (organizer)
// ---------------------------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/drawn.html")]
struct DrawnHtml<'a> {
    site: &'a SiteInfo,
    organizer_name: &'a str,
    game_name: &'a str,
    participant_count: usize,
    login_url: &'a str,
    redraw: bool,
}

#[derive(Template)]
#[template(path = "email/drawn.txt")]
struct DrawnText<'a> {
    site: &'a SiteInfo,
    organizer_name: &'a str,
    game_name: &'a str,
    participant_count: usize,
    login_url: &'a str,
    redraw: bool,
}

pub fn drawn(
    site: &SiteInfo,
    game: &Game,
    participant_count: usize,
    login_url: &str,
    redraw: bool,
) -> Result<NewEmail> {
    let html = DrawnHtml {
        site,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        participant_count,
        login_url,
        redraw,
    };
    let text = DrawnText {
        site,
        organizer_name: &game.organizer_name,
        game_name: &game.name,
        participant_count,
        login_url,
        redraw,
    };
    Ok(NewEmail {
        game_id: Some(game.id),
        participant_id: None,
        kind: EmailKind::Drawn,
        draw_round: Some(game.draw_round),
        to_email: game.organizer_email.clone(),
        to_name: Some(game.organizer_name.clone()),
        reply_to: None,
        subject: format!("Sorteio realizado: {}", game.name),
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

// ---------------------------------------------------------------------------------------------
// "Perdi meu link" recovery
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct LinkItem {
    pub game_name: String,
    pub event_date: String,
    pub url: String,
}

#[derive(Template)]
#[template(path = "email/recover.html")]
struct RecoverHtml<'a> {
    site: &'a SiteInfo,
    organized: &'a [LinkItem],
    participations: &'a [LinkItem],
}

#[derive(Template)]
#[template(path = "email/recover.txt")]
struct RecoverText<'a> {
    site: &'a SiteInfo,
    organized: &'a [LinkItem],
    participations: &'a [LinkItem],
}

pub fn recover(
    site: &SiteInfo,
    to: &EmailAddress,
    organized: &[LinkItem],
    participations: &[LinkItem],
) -> Result<NewEmail> {
    let html = RecoverHtml {
        site,
        organized,
        participations,
    };
    let text = RecoverText {
        site,
        organized,
        participations,
    };
    Ok(NewEmail {
        game_id: None,
        participant_id: None,
        kind: EmailKind::Recover,
        draw_round: None,
        to_email: to.clone(),
        to_name: None,
        reply_to: None,
        subject: format!("Seus links · {}", site.name),
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

// ---------------------------------------------------------------------------------------------
// Admin login link
// ---------------------------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "email/admin_login.html")]
struct AdminLoginHtml<'a> {
    site: &'a SiteInfo,
    login_url: &'a str,
    minutes: i64,
}

#[derive(Template)]
#[template(path = "email/admin_login.txt")]
struct AdminLoginText<'a> {
    site: &'a SiteInfo,
    login_url: &'a str,
    minutes: i64,
}

pub fn admin_login(site: &SiteInfo, to: &EmailAddress, login_url: &str) -> Result<NewEmail> {
    let html = AdminLoginHtml {
        site,
        login_url,
        minutes: ADMIN_LINK_MINUTES,
    };
    let text = AdminLoginText {
        site,
        login_url,
        minutes: ADMIN_LINK_MINUTES,
    };
    Ok(NewEmail {
        game_id: None,
        participant_id: None,
        kind: EmailKind::AdminLogin,
        draw_round: None,
        to_email: to.clone(),
        to_name: None,
        reply_to: None,
        subject: format!("Acesso ao painel · {}", site.name),
        text_body: render(&text)?,
        html_body: render(&html)?,
    })
}

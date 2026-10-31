//! Organizer pages and actions for a game.

use std::collections::HashMap;

use askama::Template;
use askama_web::WebTemplate;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use chrono::{Duration, NaiveDate};
use serde::Deserialize;

use super::AppState;
use super::auth::{self, Organizer, SessionInfo};
use super::ctx::{FlashKind, PageCtx, set_flash, take_flash};
use super::error::AppError;
use super::home::{clean, clean_text};
use crate::db;
use crate::email_addr::EmailAddress;
use crate::fmt::{self, filters};
use crate::ids::{GameId, ParticipantId, Secret};
use crate::mail::compose;
use crate::matching::{self, DrawError, MIN_PARTICIPANTS};
use crate::models::{Exclusion, Game, InviteDelivery, OutboxStatus, Participant, SessionKind};
use crate::repo;

pub const MAX_PARTICIPANTS: usize = 100;
const MAX_PARTICIPANTS_I64: i64 = MAX_PARTICIPANTS as i64;
pub const MAX_MESSAGE_CHARS: usize = 600;

// ---------------------------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------------------------

pub struct ParticipantRow {
    pub p: Participant,
    pub delivery: InviteDelivery,
    pub error: Option<String>,
    pub is_organizer: bool,
}

impl ParticipantRow {
    pub fn is_failed(&self) -> bool {
        self.delivery == InviteDelivery::Failed
    }

    pub fn is_queued(&self) -> bool {
        matches!(self.delivery, InviteDelivery::Queued | InviteDelivery::None)
    }
}

pub struct ExclusionRow {
    pub a_id: ParticipantId,
    pub b_id: ParticipantId,
    pub a_name: String,
    pub b_name: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "game.html")]
pub struct GamePage {
    pub ctx: PageCtx,
    pub game: Game,
    pub rows: Vec<ParticipantRow>,
    pub exclusions: Vec<ExclusionRow>,
    pub min_participants: usize,
    pub max_participants: usize,
    pub max_message_chars: usize,
    pub delivered: usize,
    pub failed: usize,
    pub revealed: usize,
    pub pending_reveal: usize,
}

async fn load_game(state: &AppState, session: &SessionInfo, id: GameId) -> Result<Game, AppError> {
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::games::get(&mut conn, id)
        .await?
        .filter(|g| g.organizer_email == session.email)
        .ok_or_else(|| AppError::not_found("Esse sorteio não existe ou não pertence ao seu email."))
}

pub async fn show(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let participants = repo::participants::list(&mut conn, id).await?;
    let exclusions = repo::exclusions::list(&mut conn, id).await?;
    let invites = if game.is_drawn() {
        repo::outbox::latest_invites(&mut conn, id, game.draw_round).await?
    } else {
        HashMap::new()
    };
    drop(conn);

    let names: HashMap<ParticipantId, String> = participants
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let rows: Vec<ParticipantRow> = participants
        .into_iter()
        .map(|p| {
            let (delivery, error) = match invites.get(&p.id) {
                None => (InviteDelivery::None, None),
                Some((OutboxStatus::Queued, _)) => (InviteDelivery::Queued, None),
                Some((OutboxStatus::Sent, _)) => (InviteDelivery::Sent, None),
                Some((OutboxStatus::Failed, e)) => (InviteDelivery::Failed, e.clone()),
                // Cancelled rows are filtered out by the query; a fresh one always replaces them.
                Some((OutboxStatus::Cancelled, _)) => (InviteDelivery::None, None),
            };
            let is_organizer = p.email == game.organizer_email;
            ParticipantRow {
                p,
                delivery,
                error,
                is_organizer,
            }
        })
        .collect();
    let exclusions = exclusions
        .into_iter()
        .map(|x| ExclusionRow {
            a_id: x.a_id,
            b_id: x.b_id,
            a_name: names.get(&x.a_id).cloned().unwrap_or_default(),
            b_name: names.get(&x.b_id).cloned().unwrap_or_default(),
        })
        .collect();
    let delivered = rows
        .iter()
        .filter(|r| r.delivery == InviteDelivery::Sent)
        .count();
    let failed = rows
        .iter()
        .filter(|r| r.delivery == InviteDelivery::Failed)
        .count();
    let revealed = rows.iter().filter(|r| r.p.has_revealed()).count();
    let pending_reveal = rows.len() - revealed;

    let (jar, flash) = take_flash(jar);
    Ok((
        jar,
        GamePage {
            ctx: PageCtx::new(flash, Some(session)),
            game,
            rows,
            exclusions,
            min_participants: MIN_PARTICIPANTS,
            max_participants: MAX_PARTICIPANTS,
            max_message_chars: MAX_MESSAGE_CHARS,
            delivered,
            failed,
            revealed,
            pending_reveal,
        },
    )
        .into_response())
}

fn back(jar: CookieJar, id: GameId, kind: FlashKind, msg: impl Into<String>) -> Response {
    (
        set_flash(jar, kind, msg),
        Redirect::to(&format!("/jogo/{id}")),
    )
        .into_response()
}

// ---------------------------------------------------------------------------------------------
// Edit event details
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EditForm {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub event_date: String,
    #[serde(default)]
    pub budget: String,
    #[serde(default)]
    pub organizer_name: String,
    #[serde(default)]
    pub message: String,
}

impl EditForm {
    fn from_game(g: &Game) -> Self {
        Self {
            name: g.name.clone(),
            event_date: g.event_date.format("%Y-%m-%d").to_string(),
            budget: g
                .budget_cents
                .map(|c| fmt::money(c).trim_start_matches("R$ ").to_owned())
                .unwrap_or_default(),
            organizer_name: g.organizer_name.clone(),
            message: g.message.clone().unwrap_or_default(),
        }
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "game_edit.html")]
pub struct GameEditPage {
    pub ctx: PageCtx,
    pub game: Game,
    pub form: EditForm,
    pub errors: Vec<String>,
    pub max_message_chars: usize,
}

pub async fn edit_form(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let (jar, flash) = take_flash(jar);
    let form = EditForm::from_game(&game);
    Ok((
        jar,
        GameEditPage {
            ctx: PageCtx::new(flash, Some(session)),
            game,
            form,
            errors: vec![],
            max_message_chars: MAX_MESSAGE_CHARS,
        },
    )
        .into_response())
}

pub async fn edit(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<EditForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let mut errors = Vec::new();
    let name = clean(&form.name, 80);
    if name.chars().count() < 2 {
        errors.push("Dê um nome ao evento (pelo menos 2 letras).".to_owned());
    }
    let today = fmt::today();
    let event_date = match NaiveDate::parse_from_str(form.event_date.trim(), "%Y-%m-%d") {
        Ok(d) if d < today => {
            errors.push("A data da troca já passou.".to_owned());
            None
        }
        Ok(d) if d > today + Duration::days(730) => {
            errors.push("A data da troca está longe demais (máximo 2 anos).".to_owned());
            None
        }
        Ok(d) => Some(d),
        Err(_) => {
            errors.push("Informe a data da troca de presentes.".to_owned());
            None
        }
    };
    let budget_cents = if form.budget.trim().is_empty() {
        None
    } else {
        match fmt::parse_money(&form.budget) {
            Some(c) if (100..=10_000_000).contains(&c) => Some(c),
            _ => {
                errors
                    .push("Valor sugerido inválido (use só números, ex.: 80 ou 80,50).".to_owned());
                None
            }
        }
    };
    let organizer_name = clean(&form.organizer_name, 60);
    if organizer_name.chars().count() < 2 {
        errors.push("Seu nome precisa ter pelo menos 2 letras.".to_owned());
    }
    let message = clean_text(&form.message, MAX_MESSAGE_CHARS);

    let Some(event_date) = event_date.filter(|_| errors.is_empty()) else {
        return Ok((
            jar,
            GameEditPage {
                ctx: PageCtx::new(None, Some(session)),
                game,
                form,
                errors,
                max_message_chars: MAX_MESSAGE_CHARS,
            },
        )
            .into_response());
    };

    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::games::update_details(
        &mut conn,
        id,
        repo::games::GameDetails {
            name: &name,
            event_date,
            budget_cents,
            message: message.as_deref(),
            organizer_name: &organizer_name,
        },
        db::now(),
    )
    .await?;
    Ok(back(jar, id, FlashKind::Ok, "Evento atualizado."))
}

#[derive(Debug, Deserialize)]
pub struct MessageForm {
    #[serde(default)]
    pub message: String,
}

pub async fn set_message(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<MessageForm>,
) -> Result<Response, AppError> {
    load_game(&state, &session, id).await?;
    let message = clean_text(&form.message, MAX_MESSAGE_CHARS);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::games::update_message(&mut conn, id, message.as_deref(), db::now()).await?;
    let msg = if message.is_some() {
        "Mensagem salva."
    } else {
        "Mensagem removida."
    };
    Ok(back(jar, id, FlashKind::Ok, msg))
}

// ---------------------------------------------------------------------------------------------
// Participants
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ParticipantForm {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

fn validate_person(name: &str, email: &str) -> Result<(String, EmailAddress), String> {
    let name = clean(name, 80);
    if name.chars().count() < 2 {
        return Err("O nome precisa ter pelo menos 2 letras.".to_owned());
    }
    let email = email
        .parse::<EmailAddress>()
        .map_err(|_| format!("O email de {name} parece inválido."))?;
    Ok((name, email))
}

fn new_participant(
    game_id: GameId,
    name: String,
    email: EmailAddress,
    position: i64,
) -> (Participant, Secret) {
    let secret = Secret::generate();
    let p = Participant {
        id: ParticipantId::new(),
        game_id,
        name,
        email,
        wishlist: None,
        receiver_id: None,
        reveal_token_hash: secret.hash(),
        revealed_at: None,
        position,
        created_at: db::now(),
    };
    (p, secret)
}

pub async fn add_participant(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<ParticipantForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let (name, email) = match validate_person(&form.name, &form.email) {
        Ok(v) => v,
        Err(msg) => return Ok(back(jar, id, FlashKind::Error, msg)),
    };
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    if repo::participants::count(&mut tx, id).await? >= MAX_PARTICIPANTS_I64 {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            format!("Limite de {MAX_PARTICIPANTS} participantes por sorteio."),
        ));
    }
    if repo::participants::email_exists(&mut tx, id, &email).await? {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            format!("{email} já está na lista."),
        ));
    }
    let position = repo::participants::next_position(&mut tx, id).await?;
    let (p, _) = new_participant(id, name.clone(), email, position);
    repo::participants::insert(&mut tx, &p).await?;
    repo::games::touch(&mut tx, id, db::now()).await?;

    if game.is_drawn() {
        return finish_redraw(
            &state,
            jar,
            tx,
            &game,
            format!("{name} entrou e o sorteio foi refeito. Novos convites a caminho."),
        )
        .await;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(jar, id, FlashKind::Ok, format!("{name} adicionado.")))
}

#[derive(Debug, Deserialize)]
pub struct BulkForm {
    #[serde(default)]
    pub list: String,
}

/// Parses one person per line: `Nome, email`, `Nome; email`, `Nome <email>` or just `email`.
pub fn parse_people(text: &str) -> (Vec<(String, EmailAddress)>, Vec<String>) {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = line
            .split([',', ';', '<', '>', '\t'])
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect();
        let Some(email_tok) = tokens.iter().find(|t| t.contains('@')) else {
            bad.push(line.to_owned());
            continue;
        };
        let Ok(email) = email_tok.parse::<EmailAddress>() else {
            bad.push(line.to_owned());
            continue;
        };
        let name_tokens: Vec<&str> = tokens
            .iter()
            .copied()
            .filter(|t| !t.contains('@'))
            .collect();
        let name = if name_tokens.is_empty() {
            // Derive a name from the local part: "ana.mello" → "Ana Mello".
            email
                .as_str()
                .split('@')
                .next()
                .unwrap_or("")
                .split(['.', '_', '-', '+'])
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            clean(&name_tokens.join(" "), 80)
        };
        if name.chars().count() < 2 {
            bad.push(line.to_owned());
            continue;
        }
        ok.push((name, email));
    }
    (ok, bad)
}

pub async fn add_participants_bulk(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<BulkForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let (people, bad) = parse_people(&form.list);
    if people.is_empty() && bad.is_empty() {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Cole pelo menos uma linha com nome e email.",
        ));
    }
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    let mut count = repo::participants::count(&mut tx, id).await?;
    let mut position = repo::participants::next_position(&mut tx, id).await?;
    let mut added = 0usize;
    let mut duplicates = 0usize;
    let mut over_limit = 0usize;
    for (name, email) in people {
        if count >= MAX_PARTICIPANTS_I64 {
            over_limit += 1;
            continue;
        }
        if repo::participants::email_exists(&mut tx, id, &email).await? {
            duplicates += 1;
            continue;
        }
        let (p, _) = new_participant(id, name, email, position);
        repo::participants::insert(&mut tx, &p).await?;
        position += 1;
        count += 1;
        added += 1;
    }
    repo::games::touch(&mut tx, id, db::now()).await?;

    let mut parts = vec![format!(
        "{added} pessoa{} adicionada{}.",
        plural(added),
        plural(added)
    )];
    if duplicates > 0 {
        parts.push(format!(
            "{duplicates} já estava{} na lista.",
            if duplicates == 1 { "" } else { "m" }
        ));
    }
    if over_limit > 0 {
        parts.push(format!(
            "{over_limit} ficaram de fora pelo limite de {MAX_PARTICIPANTS}."
        ));
    }
    if !bad.is_empty() {
        let shown: Vec<String> = bad
            .iter()
            .take(3)
            .map(|l| format!("“{}”", l.chars().take(40).collect::<String>()))
            .collect();
        parts.push(format!(
            "{} linha{} sem email válido: {}{}",
            bad.len(),
            plural(bad.len()),
            shown.join(", "),
            if bad.len() > 3 { "…" } else { "" }
        ));
    }
    let msg = parts.join(" ");
    let kind = if added > 0 {
        FlashKind::Ok
    } else {
        FlashKind::Error
    };

    if game.is_drawn() && added > 0 {
        return finish_redraw(
            &state,
            jar,
            tx,
            &game,
            format!("{msg} O sorteio foi refeito e novos convites estão a caminho."),
        )
        .await;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(jar, id, kind, msg))
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[derive(Debug, Deserialize)]
pub struct EditParticipantForm {
    pub pid: ParticipantId,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

pub async fn edit_participant(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<EditParticipantForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let pid = form.pid;
    let (name, email) = match validate_person(&form.name, &form.email) {
        Ok(v) => v,
        Err(msg) => return Ok(back(jar, id, FlashKind::Error, msg)),
    };
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    let Some(p) = repo::participants::get(&mut tx, pid)
        .await?
        .filter(|p| p.game_id == id)
    else {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Participante não encontrado.",
        ));
    };
    let email_changed = p.email != email;
    if email_changed && p.has_revealed() {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            format!(
                "{} já revelou quem tirou; o email não pode mais ser trocado.",
                p.name
            ),
        ));
    }
    if email_changed && repo::participants::email_exists(&mut tx, id, &email).await? {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            format!("{email} já está na lista."),
        ));
    }
    let now = db::now();
    repo::participants::update_name_email(&mut tx, pid, &name, &email).await?;
    repo::games::touch(&mut tx, id, now).await?;

    if game.is_drawn() && email_changed {
        // A new address gets its own fresh link; the one sent (or still queued) for the old
        // address dies. Same transaction as the rename, so neither can land without the other.
        let secret = Secret::generate();
        repo::participants::set_token_hash(&mut tx, pid, &secret.hash()).await?;
        repo::outbox::cancel_queued_invites_for(&mut tx, pid).await?;
        let updated = Participant {
            name: name.clone(),
            email,
            ..p
        };
        let url = state.url(&format!("/revelar/{}", secret.as_str()));
        repo::outbox::enqueue(
            &mut tx,
            &compose::invite(&state.site, &game, &updated, &url, false)?,
            now,
        )
        .await?;
        tx.commit().await.map_err(anyhow::Error::from)?;
        state.wake_mailer();
        return Ok(back(
            jar,
            id,
            FlashKind::Ok,
            format!("Dados de {name} atualizados e convite enviado para o novo email."),
        ));
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(
        jar,
        id,
        FlashKind::Ok,
        format!("Dados de {name} atualizados."),
    ))
}

pub async fn remove_participant(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path((id, pid)): Path<(GameId, ParticipantId)>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    let Some(p) = repo::participants::get(&mut tx, pid)
        .await?
        .filter(|p| p.game_id == id)
    else {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Participante não encontrado.",
        ));
    };
    repo::participants::remove(&mut tx, pid).await?;
    repo::games::touch(&mut tx, id, db::now()).await?;

    if game.is_drawn() {
        let remaining = repo::participants::count(&mut tx, id).await?;
        if usize::try_from(remaining).unwrap_or(0) < MIN_PARTICIPANTS {
            repo::participants::clear_draw(&mut tx, id).await?;
            repo::games::mark_open(&mut tx, id, db::now()).await?;
            tx.commit().await.map_err(anyhow::Error::from)?;
            return Ok(back(
                jar,
                id,
                FlashKind::Info,
                format!(
                    "{} saiu. Com menos de {MIN_PARTICIPANTS} pessoas o sorteio foi desfeito; adicione participantes e sorteie de novo.",
                    p.name
                ),
            ));
        }
        return finish_redraw(
            &state,
            jar,
            tx,
            &game,
            format!(
                "{} saiu e o sorteio foi refeito. Novos convites a caminho.",
                p.name
            ),
        )
        .await;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(
        jar,
        id,
        FlashKind::Ok,
        format!("{} removido.", p.name),
    ))
}

pub async fn resend_invite(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path((id, pid)): Path<(GameId, ParticipantId)>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    if !game.is_drawn() {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Os convites só existem depois do sorteio.",
        ));
    }
    // One transaction for the rotated link, the retired old invitation and the new one.
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    let Some(p) = repo::participants::get(&mut tx, pid)
        .await?
        .filter(|p| p.game_id == id)
    else {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Participante não encontrado.",
        ));
    };
    let now = db::now();
    let allowed = repo::rate::allow(&mut tx, &format!("resend:{pid}"), 1, 600, now).await?
        && repo::rate::allow(&mut tx, &format!("resend_day:{pid}"), 5, 86_400, now).await?;
    if !allowed {
        tx.commit().await.map_err(anyhow::Error::from)?;
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            format!(
                "Já reenviamos para {} há pouco. Aguarde 10 minutos (máximo de 5 por dia).",
                p.name
            ),
        ));
    }
    let secret = Secret::generate();
    repo::participants::set_token_hash(&mut tx, pid, &secret.hash()).await?;
    repo::outbox::cancel_queued_invites_for(&mut tx, pid).await?;
    let url = state.url(&format!("/revelar/{}", secret.as_str()));
    repo::outbox::enqueue(
        &mut tx,
        &compose::invite(&state.site, &game, &p, &url, false)?,
        now,
    )
    .await?;
    tx.commit().await.map_err(anyhow::Error::from)?;
    state.wake_mailer();
    Ok(back(
        jar,
        id,
        FlashKind::Ok,
        format!(
            "Convite reenviado para {} ({}). O link anterior deixou de valer.",
            p.name, p.email
        ),
    ))
}

pub async fn resend_pending(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    if !game.is_drawn() {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Os convites só existem depois do sorteio.",
        ));
    }
    let now = db::now();
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    if !repo::rate::allow(&mut tx, &format!("resend_all:{id}"), 1, 3600, now).await? {
        tx.commit().await.map_err(anyhow::Error::from)?;
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "O reenvio em massa pode ser feito uma vez por hora.",
        ));
    }
    let pending: Vec<Participant> = repo::participants::list(&mut tx, id)
        .await?
        .into_iter()
        .filter(|p| !p.has_revealed())
        .collect();
    for p in &pending {
        let secret = Secret::generate();
        repo::participants::set_token_hash(&mut tx, p.id, &secret.hash()).await?;
        repo::outbox::cancel_queued_invites_for(&mut tx, p.id).await?;
        let url = state.url(&format!("/revelar/{}", secret.as_str()));
        repo::outbox::enqueue(
            &mut tx,
            &compose::invite(&state.site, &game, p, &url, false)?,
            now,
        )
        .await?;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    state.wake_mailer();
    Ok(back(
        jar,
        id,
        FlashKind::Ok,
        format!(
            "Convites reenviados para {} pessoa{} que ainda não revelaram.",
            pending.len(),
            plural(pending.len())
        ),
    ))
}

// ---------------------------------------------------------------------------------------------
// Exclusions
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ExclusionForm {
    pub a: ParticipantId,
    pub b: ParticipantId,
}

pub async fn add_exclusion(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<ExclusionForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    if form.a == form.b {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Escolha duas pessoas diferentes.",
        ));
    }
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    let a = repo::participants::get(&mut tx, form.a)
        .await?
        .filter(|p| p.game_id == id);
    let b = repo::participants::get(&mut tx, form.b)
        .await?
        .filter(|p| p.game_id == id);
    let (Some(a), Some(b)) = (a, b) else {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Participante não encontrado.",
        ));
    };
    if !repo::exclusions::insert(&mut tx, id, Exclusion::new(a.id, b.id)).await? {
        return Ok(back(
            jar,
            id,
            FlashKind::Info,
            format!("{} e {} já não podiam tirar um ao outro.", a.name, b.name),
        ));
    }
    repo::games::touch(&mut tx, id, db::now()).await?;
    if game.is_drawn() {
        return finish_redraw(
            &state,
            jar,
            tx,
            &game,
            "Restrição adicionada e sorteio refeito. Novos convites a caminho.".to_owned(),
        )
        .await;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(
        jar,
        id,
        FlashKind::Ok,
        format!("{} e {} não vão tirar um ao outro.", a.name, b.name),
    ))
}

pub async fn remove_exclusion(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<ExclusionForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    if !repo::exclusions::remove(&mut tx, id, Exclusion::new(form.a, form.b)).await? {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Essa restrição não existe mais.",
        ));
    }
    repo::games::touch(&mut tx, id, db::now()).await?;
    if game.is_drawn() {
        return finish_redraw(
            &state,
            jar,
            tx,
            &game,
            "Restrição removida e sorteio refeito. Novos convites a caminho.".to_owned(),
        )
        .await;
    }
    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(back(jar, id, FlashKind::Ok, "Restrição removida."))
}

// ---------------------------------------------------------------------------------------------
// Draw
// ---------------------------------------------------------------------------------------------

/// Runs the draw for `game` inside `tx`: assigns receivers, rotates reveal links, bumps the
/// round, retires invitations from earlier rounds that were still waiting to be sent, and
/// queues the new invitations plus the organizer's confirmation. All of it lands in the same
/// transaction, so a crash can never leave a draw without its emails or emails carrying links
/// that no longer work. Does not commit; the caller commits and then calls
/// [`AppState::wake_mailer`].
pub async fn draw_in_tx(
    state: &AppState,
    tx: &mut sqlx::SqliteConnection,
    game: &Game,
) -> Result<Result<Game, DrawError>, AppError> {
    let participants = repo::participants::list(tx, game.id).await?;
    let exclusions = repo::exclusions::list(tx, game.id).await?;
    let ids: Vec<ParticipantId> = participants.iter().map(|p| p.id).collect();
    let pairs = match matching::draw(&ids, &exclusions) {
        Ok(p) => p,
        Err(e) => return Ok(Err(e)),
    };
    let receiver_of: HashMap<ParticipantId, ParticipantId> = pairs.into_iter().collect();

    let now = db::now();
    let round = repo::games::mark_drawn(tx, game.id, now).await?;
    let game = Game {
        status: crate::models::GameStatus::Drawn,
        draw_round: round,
        drawn_at: Some(now),
        updated_at: now,
        ..game.clone()
    };
    let redraw = round > 1;
    repo::outbox::cancel_queued_before_round(tx, game.id, round).await?;

    let mut assignments = Vec::with_capacity(participants.len());
    for p in participants {
        let secret = Secret::generate();
        let receiver = receiver_of[&p.id];
        assignments.push((p.id, receiver, secret.hash()));
        let p = Participant {
            receiver_id: Some(receiver),
            reveal_token_hash: secret.hash(),
            revealed_at: None,
            ..p
        };
        let url = state.url(&format!("/revelar/{}", secret.as_str()));
        let invite = compose::invite(&state.site, &game, &p, &url, redraw)?;
        repo::outbox::enqueue(tx, &invite, now).await?;
    }
    repo::participants::apply_draw(tx, &assignments).await?;

    let organizer_login = auth::issue_login_token(
        tx,
        &game.organizer_email,
        SessionKind::Organizer,
        auth::ORGANIZER_LINK_SECS,
    )
    .await?;
    let login_url = auth::login_url(state, &organizer_login, Some(&format!("/jogo/{}", game.id)));
    let confirmation = compose::drawn(&state.site, &game, assignments.len(), &login_url, redraw)?;
    repo::outbox::enqueue(tx, &confirmation, now).await?;
    Ok(Ok(game))
}

fn draw_error_message(e: DrawError) -> String {
    match e {
        DrawError::TooFew => format!("São necessários pelo menos {MIN_PARTICIPANTS} participantes para sortear."),
        DrawError::Infeasible => "Com essas restrições não existe combinação possível. Remova alguma restrição ou adicione participantes.".to_owned(),
    }
}

/// Completes a mutation that requires a redraw: draws (and queues the emails) inside `tx`,
/// commits, wakes the mailer. On an impossible draw the transaction is dropped (rolled back)
/// and the organizer sees why.
async fn finish_redraw(
    state: &AppState,
    jar: CookieJar,
    mut tx: sqlx::Transaction<'_, sqlx::Sqlite>,
    game: &Game,
    success_msg: String,
) -> Result<Response, AppError> {
    match draw_in_tx(state, &mut tx, game).await? {
        Ok(_) => {
            tx.commit().await.map_err(anyhow::Error::from)?;
            state.wake_mailer();
            Ok(back(jar, game.id, FlashKind::Ok, success_msg))
        }
        Err(e) => Ok(back(
            jar,
            game.id,
            FlashKind::Error,
            format!(
                "Nada foi alterado: {}",
                draw_error_message(e).to_lowercase()
            ),
        )),
    }
}

pub async fn draw(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    let mut tx = state.db.begin().await.map_err(anyhow::Error::from)?;
    match draw_in_tx(&state, &mut tx, &game).await? {
        Ok(game) => {
            tx.commit().await.map_err(anyhow::Error::from)?;
            state.wake_mailer();
            let msg = if game.draw_round > 1 {
                "Sorteio refeito! Todos recebem um novo convite; os links antigos deixaram de valer."
            } else {
                "Sorteio realizado! Os convites estão sendo enviados."
            };
            Ok(back(jar, id, FlashKind::Ok, msg))
        }
        Err(e) => Ok(back(jar, id, FlashKind::Error, draw_error_message(e))),
    }
}

// ---------------------------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeleteForm {
    #[serde(default)]
    pub confirm_name: String,
}

pub async fn delete(
    State(state): State<AppState>,
    Organizer(session): Organizer,
    Path(id): Path<GameId>,
    jar: CookieJar,
    Form(form): Form<DeleteForm>,
) -> Result<Response, AppError> {
    let game = load_game(&state, &session, id).await?;
    if clean(&form.confirm_name, 80).to_lowercase() != game.name.to_lowercase() {
        return Ok(back(
            jar,
            id,
            FlashKind::Error,
            "Para excluir, digite o nome do evento exatamente como aparece.",
        ));
    }
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::games::delete(&mut conn, id).await?;
    let jar = set_flash(
        jar,
        FlashKind::Info,
        format!("O sorteio “{}” foi excluído.", game.name),
    );
    Ok((jar, Redirect::to("/meus")).into_response())
}

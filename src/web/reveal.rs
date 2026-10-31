//! Participant pages: the secret link, the reveal itself, gift hints and the calendar file.

use askama::Template;
use askama_web::WebTemplate;
use axum::Form;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use super::AppState;
use super::ctx::{FlashKind, PageCtx, set_flash, take_flash};
use super::error::AppError;
use super::home::clean_text;
use crate::db;
use crate::fmt::filters;
use crate::ids::Secret;
use crate::mail::ics;
use crate::models::{Game, Participant};
use crate::repo;

pub const MAX_WISHLIST_CHARS: usize = 500;

pub struct Receiver {
    pub name: String,
    pub wishlist: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "reveal.html")]
pub struct RevealPage {
    pub ctx: PageCtx,
    pub token: String,
    pub game: Game,
    pub me: Participant,
    pub receiver: Option<Receiver>,
    pub max_wishlist_chars: usize,
}

async fn load(state: &AppState, token: &str) -> Result<(Participant, Game), AppError> {
    let invalid = || {
        AppError::not_found(
            "Este link não é válido ou foi substituído por um mais novo — quando um convite é \
             reenviado, o link anterior deixa de valer. Peça um novo em “Perdi meu link”.",
        )
    };
    let secret: Secret = token.parse().map_err(|_| invalid())?;
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let me = repo::participants::get_by_token_hash(&mut conn, &secret.hash())
        .await?
        .ok_or_else(invalid)?;
    let game = repo::games::get(&mut conn, me.game_id)
        .await?
        .ok_or_else(invalid)?;
    Ok((me, game))
}

pub async fn show(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let (me, game) = load(&state, &token).await?;
    let (jar, flash) = take_flash(jar);
    Ok((
        jar,
        RevealPage {
            ctx: PageCtx::new(flash, None),
            token,
            game,
            me,
            receiver: None,
            max_wishlist_chars: MAX_WISHLIST_CHARS,
        },
    )
        .into_response())
}

pub async fn reveal(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let (me, game) = load(&state, &token).await?;
    let Some(receiver_id) = me.receiver_id.filter(|_| game.is_drawn()) else {
        let jar = set_flash(
            jar,
            FlashKind::Info,
            "O sorteio ainda não aconteceu. Você receberá um email quando acontecer.",
        );
        return Ok((jar, Redirect::to(&format!("/revelar/{token}"))).into_response());
    };
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    let receiver = repo::participants::get(&mut conn, receiver_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("receiver {receiver_id} missing for participant {}", me.id)
        })?;
    repo::participants::mark_revealed(&mut conn, me.id, db::now()).await?;
    drop(conn);
    let me = Participant {
        revealed_at: me.revealed_at.or(Some(db::now())),
        ..me
    };
    let (jar, flash) = take_flash(jar);
    Ok((
        jar,
        RevealPage {
            ctx: PageCtx::new(flash, None),
            token,
            game,
            me,
            receiver: Some(Receiver {
                name: receiver.name,
                wishlist: receiver.wishlist,
            }),
            max_wishlist_chars: MAX_WISHLIST_CHARS,
        },
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
pub struct WishlistForm {
    #[serde(default)]
    pub wishlist: String,
}

pub async fn save_wishlist(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
    Form(form): Form<WishlistForm>,
) -> Result<Response, AppError> {
    let (me, _game) = load(&state, &token).await?;
    let wishlist = clean_text(&form.wishlist, MAX_WISHLIST_CHARS);
    let mut conn = state.db.acquire().await.map_err(anyhow::Error::from)?;
    repo::participants::set_wishlist(&mut conn, me.id, wishlist.as_deref()).await?;
    let msg = if wishlist.is_some() {
        "Dicas salvas. Quem tirou você vai vê-las ao revelar."
    } else {
        "Dicas apagadas."
    };
    let jar = set_flash(jar, FlashKind::Ok, msg);
    Ok((jar, Redirect::to(&format!("/revelar/{token}"))).into_response())
}

pub async fn ics(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, AppError> {
    let (_me, game) = load(&state, &token).await?;
    let domain = state
        .cfg
        .base_url
        .host_str()
        .unwrap_or("amigoocultosimples.app")
        .to_owned();
    let body = ics::for_game(&game, &state.site.url, &domain);
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/calendar; charset=utf-8".to_owned(),
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"amigo-oculto.ics\"".to_owned(),
            ),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
        ],
        body,
    )
        .into_response())
}

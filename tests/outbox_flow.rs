mod common;

use std::time::Duration;

use amigo_oculto::cleanup;
use amigo_oculto::ids::GameId;
use amigo_oculto::mail::{Mailer, worker};
use amigo_oculto::models::{EmailKind, Game, GameStatus};
use amigo_oculto::repo;
use amigo_oculto::repo::outbox::NewEmail;
use common::*;

fn sample_email() -> NewEmail {
    NewEmail {
        game_id: None,
        participant_id: None,
        kind: EmailKind::Recover,
        draw_round: None,
        to_email: "ana@exemplo.com".parse().unwrap(),
        to_name: Some("Ana".into()),
        reply_to: None,
        subject: "Teste".into(),
        text_body: "Corpo".into(),
        html_body: "<p>Corpo</p>".into(),
    }
}

async fn queue(state: &amigo_oculto::AppState) {
    let mut conn = state.db.acquire().await.unwrap();
    repo::outbox::enqueue(&mut conn, &sample_email(), amigo_oculto::db::now())
        .await
        .unwrap();
}

#[tokio::test]
async fn worker_sends_queued_mail_once() {
    let app = app().await;
    queue(&app.state).await;
    assert_eq!(app.take_mail().await.len(), 1);
    // Nothing left to send: idempotent.
    assert!(app.take_mail().await.is_empty());
}

#[tokio::test]
async fn worker_retries_transient_failures_with_backoff_and_gives_up_eventually() {
    let app = app().await;
    let failing = Mailer::Failing { permanent: false };
    queue(&app.state).await;

    // Each pass fails transiently, schedules a retry, and the clock is forced forward so the
    // next pass sees it due again. After the last backoff step it gives up.
    for pass in 1..=6 {
        let n = worker::process_due(&app.state.db, &failing).await.unwrap();
        assert_eq!(n, 1, "pass {pass} attempts the email");
        shift_outbox_clock(&app.state).await;
    }
    assert_eq!(
        outbox_statuses(&app.state).await,
        vec!["failed".to_owned()],
        "given up after exhausting backoff"
    );
}

/// Forces queued retries to become due (test time is not real time).
async fn shift_outbox_clock(state: &amigo_oculto::AppState) {
    let mut conn = state.db.acquire().await.unwrap();
    sqlx::query("UPDATE outbox SET next_attempt_at = 0 WHERE status = 'queued'")
        .execute(&mut *conn)
        .await
        .unwrap();
}

async fn outbox_statuses(state: &amigo_oculto::AppState) -> Vec<String> {
    let mut conn = state.db.acquire().await.unwrap();
    let rows: Vec<(String,)> = sqlx::query_as("SELECT status FROM outbox ORDER BY created_at")
        .fetch_all(&mut *conn)
        .await
        .unwrap();
    rows.into_iter().map(|(s,)| s).collect()
}

#[tokio::test]
async fn permanent_failures_are_not_retried() {
    let app = app().await;
    queue(&app.state).await;
    let permanent = Mailer::Failing { permanent: true };
    worker::process_due(&app.state.db, &permanent)
        .await
        .unwrap();
    assert_eq!(outbox_statuses(&app.state).await, vec!["failed".to_owned()]);
}

#[tokio::test]
async fn cleanup_deletes_old_games_and_expired_rows() {
    let app = app().await;
    let old = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let fresh = chrono::NaiveDate::from_ymd_opt(2100, 1, 1).unwrap();
    for (date, tag) in [(old, "velho"), (fresh, "novo")] {
        let g = Game {
            id: GameId::new(),
            name: format!("Sorteio {tag}"),
            event_date: date,
            budget_cents: None,
            message: None,
            organizer_name: "Ana".into(),
            organizer_email: "ana@exemplo.com".parse().unwrap(),
            status: GameStatus::Open,
            draw_round: 0,
            drawn_at: None,
            created_at: 0,
            updated_at: 0,
        };
        let mut conn = app.state.db.acquire().await.unwrap();
        repo::games::insert(&mut conn, &g).await.unwrap();
    }
    cleanup::run_once(&app.state.db, 90).await.unwrap();
    let mut conn = app.state.db.acquire().await.unwrap();
    let names: Vec<(String,)> = sqlx::query_as("SELECT name FROM games ORDER BY name")
        .fetch_all(&mut *conn)
        .await
        .unwrap();
    assert_eq!(names, vec![("Sorteio novo".to_owned(),)]);
}

#[tokio::test]
async fn worker_loop_terminates_on_cancel() {
    let app = app().await;
    let failing = Mailer::Failing { permanent: true };
    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let cancel = tokio_util::sync::CancellationToken::new();
    let handle = tokio::spawn(worker::run(
        app.state.db.clone(),
        failing,
        notify,
        cancel.clone(),
    ));
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .unwrap()
        .unwrap();
}

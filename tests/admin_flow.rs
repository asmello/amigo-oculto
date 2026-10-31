mod common;

use common::*;

#[tokio::test]
async fn admin_login_is_magic_link_for_configured_emails_only() {
    let mut app = app().await;
    let game_id = create_game(&mut app, "Natal", "ana@exemplo.com").await;
    app.take_mail().await;
    app.clear_cookies();

    // Not logged in: login form; the organizer session does not count as admin either.
    app.get("/admin")
        .await
        .assert_ok()
        .assert_contains("Enviar link")
        .assert_not_contains("Natal");

    // Non-admin email: same "sent" page, no email.
    app.post("/admin/entrar", &[("email", "ana@exemplo.com")])
        .await
        .assert_ok()
        .assert_contains("um link chega em instantes");
    assert!(app.take_mail().await.is_empty());

    // Admin email: link arrives, is single-use, opens the panel.
    app.post("/admin/entrar", &[("email", "admin@exemplo.com")])
        .await
        .assert_ok();
    let mail = app.take_mail().await;
    assert_eq!(mail.len(), 1);
    let login = path_of(&link_in(&mail[0], "/entrar/"));
    app.get(&login).await.assert_redirect("/admin");
    let panel = app.get("/admin").await;
    panel
        .assert_ok()
        .assert_contains("Natal")
        .assert_contains("ana@exemplo.com")
        .assert_contains("1 sorteio<");
    assert_eq!(
        app.get(&login).await.status,
        404,
        "admin links are single-use"
    );

    // The panel never shows reveal tokens or matches; deletion works.
    panel.assert_not_contains("/revelar/");
    app.post(&format!("/admin/jogos/{game_id}/excluir"), &[])
        .await
        .assert_redirect("/admin");
    app.get("/admin")
        .await
        .assert_contains("excluído")
        .assert_contains("0 sorteios");

    // Admin session cannot act as an organizer.
    app.get("/meus").await.assert_redirect_starts("/recuperar");
}

#[tokio::test]
async fn admin_actions_require_admin_session() {
    let mut app = app().await;
    let game_id = create_game(&mut app, "Natal", "ana@exemplo.com").await;
    // Organizer is logged in, but is not an admin.
    app.post(&format!("/admin/jogos/{game_id}/excluir"), &[])
        .await
        .assert_redirect("/admin");
    app.get(&format!("/jogo/{game_id}")).await.assert_ok();
}

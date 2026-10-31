mod common;

use common::*;

#[tokio::test]
async fn home_renders_with_security_headers() {
    let mut app = app().await;
    let res = app.get("/").await;
    res.assert_ok().assert_contains("Criar sorteio");
    assert!(res.headers.get("content-security-policy").is_some());
    assert_eq!(res.headers.get("x-frame-options").unwrap(), "DENY");
    assert_eq!(res.headers.get("referrer-policy").unwrap(), "no-referrer");
}

#[tokio::test]
async fn healthz_and_static_assets() {
    let mut app = app().await;
    app.get("/healthz").await.assert_ok().assert_contains("ok");
    let css = app.get("/static/app.css").await;
    css.assert_ok();
    assert!(
        css.headers
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/css")
    );
    assert_eq!(app.get("/static/nope.css").await.status, 404);
    assert_eq!(app.get("/nao-existe").await.status, 404);
}

#[tokio::test]
async fn create_form_validates_input() {
    let mut app = app().await;
    let res = app
        .post(
            "/criar",
            &[
                ("name", "N"),
                ("event_date", "2001-01-01"),
                ("budget", "abc"),
                ("organizer_name", ""),
                ("organizer_email", "nope"),
            ],
        )
        .await;
    res.assert_ok()
        .assert_contains("pelo menos 2 letras")
        .assert_contains("já passou")
        .assert_contains("Valor sugerido inválido")
        .assert_contains("Diga seu nome")
        .assert_contains("Confira o seu email");
    assert!(app.take_mail().await.is_empty());
}

#[tokio::test]
async fn create_verify_and_welcome_flow() {
    let mut app = app().await;
    let res = app
        .post(
            "/criar",
            &[
                ("name", "Natal da família"),
                ("event_date", "2027-12-24"),
                ("budget", "R$ 80,50"),
                ("organizer_name", "Ana Mello"),
                ("organizer_email", "Ana@Exemplo.com"),
            ],
        )
        .await;
    let verify_path = res.assert_redirect_starts("/verificar/");

    // Verification page shows the normalized address and asks for the code.
    app.get(&verify_path)
        .await
        .assert_ok()
        .assert_contains("ana@exemplo.com")
        .assert_contains("Código");

    let mail = app.take_mail().await;
    assert_eq!(mail.len(), 1);
    let code = code_in(&mail[0]);
    assert!(mail[0].subject.contains(&code));

    // Wrong code is rejected and counted.
    let wrong = if code == "000000" { "111111" } else { "000000" };
    app.post(&verify_path, &[("code", wrong)])
        .await
        .assert_ok()
        .assert_contains("Código incorreto");

    // Right code creates the game, logs the organizer in and sends the welcome email.
    let res = app.post(&verify_path, &[("code", &code)]).await;
    let game_path = res.assert_redirect_starts("/jogo/");
    assert!(
        app.jar.iter().any(|(k, _)| k == "sid"),
        "session cookie set"
    );

    let page = app.get(&game_path).await;
    page.assert_ok()
        .assert_contains("Natal da família")
        .assert_contains("até R$ 80,50")
        .assert_contains("24 de dezembro de 2027")
        .assert_contains("Email confirmado e sorteio criado");

    let mail = app.take_mail().await;
    let welcome = mail
        .iter()
        .find(|m| m.subject.contains("foi criado"))
        .expect("welcome email");
    let login = link_in(welcome, "/entrar/");
    assert!(login.contains("next=%2Fjogo%2F"));

    // The verification cannot be replayed.
    assert_eq!(app.post(&verify_path, &[("code", &code)]).await.status, 404);

    // The emailed login link works from a fresh browser.
    app.clear_cookies();
    app.get(&game_path)
        .await
        .assert_redirect_starts("/recuperar?next=");
    app.get(&path_of(&login)).await.assert_redirect(&game_path);
    app.get(&game_path)
        .await
        .assert_ok()
        .assert_contains("Natal da família");

    // Logout clears the session.
    app.post("/sair", &[]).await.assert_redirect("/");
    app.get(&game_path)
        .await
        .assert_redirect_starts("/recuperar");
}

#[tokio::test]
async fn verification_locks_after_too_many_attempts_and_can_be_resent() {
    let mut app = app().await;
    let res = app
        .post(
            "/criar",
            &[
                ("name", "Teste"),
                ("event_date", "2027-01-09"),
                ("budget", ""),
                ("organizer_name", "Ana"),
                ("organizer_email", "ana@exemplo.com"),
            ],
        )
        .await;
    let verify_path = res.assert_redirect_starts("/verificar/");
    let code = code_in(&app.take_mail().await[0]);
    let wrong = if code == "000000" { "111111" } else { "000000" };
    for _ in 0..5 {
        app.post(&verify_path, &[("code", wrong)]).await.assert_ok();
    }
    // Even the right code is refused now.
    app.post(&verify_path, &[("code", &code)])
        .await
        .assert_ok()
        .assert_contains("Muitas tentativas");

    // Resend issues a fresh code that works.
    app.post(&format!("{verify_path}/reenviar"), &[])
        .await
        .assert_redirect(&verify_path);
    let code2 = code_in(&app.take_mail().await[0]);
    app.post(&verify_path, &[("code", &code2)])
        .await
        .assert_redirect_starts("/jogo/");
}

#[tokio::test]
async fn verification_codes_are_rate_limited_per_email() {
    let mut app = app().await;
    for _ in 0..3 {
        app.post(
            "/criar",
            &[
                ("name", "Teste"),
                ("event_date", "2027-01-09"),
                ("budget", ""),
                ("organizer_name", "Ana"),
                ("organizer_email", "ana@exemplo.com"),
            ],
        )
        .await
        .assert_redirect_starts("/verificar/");
    }
    let res = app
        .post(
            "/criar",
            &[
                ("name", "Teste"),
                ("event_date", "2027-01-09"),
                ("budget", ""),
                ("organizer_name", "Ana"),
                ("organizer_email", "ana@exemplo.com"),
            ],
        )
        .await;
    res.assert_ok().assert_contains("Muitos códigos");
    assert_eq!(app.take_mail().await.len(), 3);
}

#[tokio::test]
async fn cross_site_posts_are_rejected() {
    let mut app = app().await;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/criar")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", "https://evil.example")
        .header("sec-fetch-site", "cross-site")
        .body(axum::body::Body::from("name=x"))
        .unwrap();
    assert_eq!(app.post_raw(req).await.status, 403);
}

#[tokio::test]
async fn recover_sends_links_without_revealing_whether_email_exists() {
    let mut app = app().await;
    let game_id = create_game(&mut app, "Natal", "ana@exemplo.com").await;
    app.take_mail().await;

    // Unknown email: same page, no email.
    app.post("/recuperar", &[("email", "ninguem@exemplo.com")])
        .await
        .assert_ok()
        .assert_contains("Verifique seu email");
    assert!(app.take_mail().await.is_empty());

    // Known organizer: gets a login link that opens the game.
    app.clear_cookies();
    app.post("/recuperar", &[("email", "ANA@exemplo.com")])
        .await
        .assert_ok();
    let mail = app.take_mail().await;
    assert_eq!(mail.len(), 1);
    assert!(mail[0].text_body.contains("Natal"));
    let login = link_in(&mail[0], "/entrar/");
    app.get(&path_of(&login))
        .await
        .assert_redirect(&format!("/jogo/{game_id}"));
    app.get("/meus").await.assert_ok().assert_contains("Natal");
}

#[tokio::test]
async fn login_link_cannot_redirect_off_site() {
    let mut app = app().await;
    create_game(&mut app, "Natal", "ana@exemplo.com").await;
    app.take_mail().await;
    app.clear_cookies();
    app.post("/recuperar", &[("email", "ana@exemplo.com")])
        .await
        .assert_ok();
    let login = link_in(&app.take_mail().await[0], "/entrar/");
    let token_path = path_of(&login);
    let token_only = token_path.split('?').next().unwrap().to_owned();
    app.get(&format!("{token_only}?next=https://evil.example"))
        .await
        .assert_redirect("/meus");
    app.get(&format!("{token_only}?next=//evil.example"))
        .await
        .assert_redirect("/meus");
    assert_eq!(app.get("/entrar/not-a-token").await.status, 404);
}

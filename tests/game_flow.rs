mod common;

use std::collections::{HashMap, HashSet};

use common::*;

const PEOPLE: [(&str, &str); 5] = [
    ("Ana Mello", "ana@exemplo.com"),
    ("Bruno Mello", "bruno@exemplo.com"),
    ("Carla Souza", "carla@exemplo.com"),
    ("Daniel Mello", "daniel@exemplo.com"),
    ("Eduarda Lima", "duda@exemplo.com"),
];

async fn game_with_people(app: &mut App, n: usize) -> String {
    let game_id = create_game(app, "Natal da família", "ana@exemplo.com").await;
    for (name, email) in PEOPLE.iter().take(n) {
        add_participant(app, &game_id, name, email).await;
    }
    app.take_mail().await;
    game_id
}

/// Reveal links from invitation emails, keyed by recipient address.
async fn reveal_links(app: &App) -> HashMap<String, String> {
    app.take_mail()
        .await
        .into_iter()
        .filter(|m| m.text_body.contains("/revelar/"))
        .map(|m| (m.to.clone(), path_of(&link_in(&m, "/revelar/"))))
        .collect()
}

/// Follows every reveal link and returns giver email → receiver name.
async fn reveal_all(app: &mut App, links: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (email, path) in links {
        let page = app.post(path, &[]).await;
        page.assert_ok().assert_contains("você tirou");
        let name = page
            .body
            .split("id=\"big-name\">")
            .nth(1)
            .and_then(|s| s.split('<').next())
            .expect("big name")
            .trim()
            .to_owned();
        out.insert(email.clone(), name);
    }
    out
}

fn assert_valid_derangement(pairs: &HashMap<String, String>) {
    let names: HashMap<&str, &str> = PEOPLE.iter().map(|(n, e)| (*e, *n)).collect();
    let receivers: HashSet<&str> = pairs.values().map(String::as_str).collect();
    assert_eq!(receivers.len(), pairs.len(), "every receiver appears once");
    for (giver_email, receiver) in pairs {
        assert_ne!(
            names[giver_email.as_str()],
            receiver,
            "nobody draws themselves"
        );
    }
}

#[tokio::test]
async fn participants_crud_before_draw() {
    let mut app = app().await;
    let game_id = create_game(&mut app, "Natal", "ana@exemplo.com").await;
    app.take_mail().await; // discard the welcome email
    let base = format!("/jogo/{game_id}");

    add_participant(&mut app, &game_id, "Ana Mello", "ana@exemplo.com").await;
    app.get(&base)
        .await
        .assert_ok()
        .assert_contains("Ana Mello")
        .assert_contains("você");

    // Duplicate email, bad email and short name are refused with a flash message.
    app.post(
        &format!("{base}/participantes"),
        &[("name", "Outra Ana"), ("email", "ANA@exemplo.com")],
    )
    .await
    .assert_redirect(&base);
    app.get(&base).await.assert_contains("já está na lista");
    app.post(
        &format!("{base}/participantes"),
        &[("name", "Bruno"), ("email", "bruno")],
    )
    .await
    .assert_redirect(&base);
    app.get(&base).await.assert_contains("parece inválido");
    app.post(
        &format!("{base}/participantes"),
        &[("name", "B"), ("email", "b@exemplo.com")],
    )
    .await
    .assert_redirect(&base);
    app.get(&base).await.assert_contains("pelo menos 2 letras");

    // Bulk add with mixed formats, a duplicate and a bad line.
    let list = "Bruno Mello, bruno@exemplo.com\nCarla Souza <carla@exemplo.com>\nana@exemplo.com\nsem email nenhum\ndaniel.mello@exemplo.com";
    app.post(&format!("{base}/participantes/lote"), &[("list", list)])
        .await
        .assert_redirect(&base);
    let page = app.get(&base).await;
    page.assert_contains("3 pessoas adicionadas")
        .assert_contains("1 já estava na lista")
        .assert_contains("1 linha sem email válido")
        .assert_contains("Bruno Mello")
        .assert_contains("Carla Souza")
        .assert_contains("Daniel Mello");

    // Edit and remove.
    let pid = participant_id(&page.body, "Carla Souza");
    app.post(
        &format!("{base}/participantes/editar"),
        &[
            ("pid", &pid),
            ("name", "Carla S. Souza"),
            ("email", "carla.souza@exemplo.com"),
        ],
    )
    .await
    .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Carla S. Souza")
        .assert_contains("carla.souza@exemplo.com");
    app.post(&format!("{base}/participantes/{pid}/remover"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_not_contains("carla.souza@exemplo.com")
        .assert_contains("removido");

    // Nothing was emailed for any of this.
    assert!(app.take_mail().await.is_empty());
}

fn participant_id(body: &str, name: &str) -> String {
    let marker = format!("data-name=\"{name}\"");
    let pos = body.find(&marker).expect("participant row");
    let before = &body[..pos];
    let id_start = before.rfind("data-id=\"").expect("data-id") + 9;
    before[id_start..].split('"').next().unwrap().to_owned()
}

#[tokio::test]
async fn draw_requires_minimum_and_respects_exclusions() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 2).await;
    let base = format!("/jogo/{game_id}");

    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("pelo menos 3 participantes");
    assert!(app.take_mail().await.is_empty());

    // Three people with one couple exclusion has no solution at all: the only derangements
    // of three people are the two 3-cycles, and both use a forbidden edge.
    add_participant(&mut app, &game_id, "Carla Souza", "carla@exemplo.com").await;
    let page = app.get(&base).await;
    let ana = participant_id(&page.body, "Ana Mello");
    let bruno = participant_id(&page.body, "Bruno Mello");
    app.post(&format!("{base}/restricoes"), &[("a", &ana), ("b", &bruno)])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Ana Mello ↔ Bruno Mello");
    // Same pair again (reversed) is reported as already present.
    app.post(&format!("{base}/restricoes"), &[("a", &bruno), ("b", &ana)])
        .await
        .assert_redirect(&base);
    app.get(&base).await.assert_contains("já não podiam");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("não existe combinação possível");

    // A fourth person makes it solvable; the couple never draws each other.
    add_participant(&mut app, &game_id, "Daniel Mello", "daniel@exemplo.com").await;
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    let page = app.get(&base).await;
    page.assert_contains("Sorteado")
        .assert_not_contains("Aguardando sorteio");

    let mail = app.take_mail().await;
    assert_eq!(
        mail.iter()
            .filter(|m| m.text_body.contains("/revelar/"))
            .count(),
        4
    );
    let drawn = mail
        .iter()
        .find(|m| m.subject.starts_with("Sorteio realizado"))
        .expect("organizer email");
    assert_eq!(drawn.to, "ana@exemplo.com");

    let links: HashMap<String, String> = mail
        .iter()
        .filter(|m| m.text_body.contains("/revelar/"))
        .map(|m| (m.to.clone(), path_of(&link_in(m, "/revelar/"))))
        .collect();
    let pairs = reveal_all(&mut app, &links).await;
    assert_ne!(
        pairs["ana@exemplo.com"], "Bruno Mello",
        "Ana does not draw her partner"
    );
    assert_ne!(
        pairs["bruno@exemplo.com"], "Ana Mello",
        "Bruno does not draw his partner"
    );
    assert_valid_derangement(&pairs);
}

#[tokio::test]
async fn infeasible_exclusions_are_refused_without_changing_anything() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    let page = app.get(&base).await;
    let ana = participant_id(&page.body, "Ana Mello");
    let bruno = participant_id(&page.body, "Bruno Mello");
    let carla = participant_id(&page.body, "Carla Souza");
    app.post(&format!("{base}/restricoes"), &[("a", &ana), ("b", &bruno)])
        .await
        .assert_redirect(&base);
    app.post(&format!("{base}/restricoes"), &[("a", &ana), ("b", &carla)])
        .await
        .assert_redirect(&base);
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("não existe combinação possível")
        .assert_contains("Aguardando sorteio");
    assert!(app.take_mail().await.is_empty());
}

#[tokio::test]
async fn reveal_is_post_only_and_tracks_status() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    let links = reveal_links(&app).await;
    let bruno = &links["bruno@exemplo.com"];

    // GET shows the invitation but not the name; nothing is marked.
    app.clear_cookies();
    let page = app.get(bruno).await;
    page.assert_ok()
        .assert_contains("Revelar meu amigo oculto")
        .assert_not_contains("você tirou");
    assert_eq!(app.get("/revelar/nope").await.status, 404);

    // POST reveals; the organizer sees "Revelou" for Bruno only.
    let page = app.post(bruno, &[]).await;
    page.assert_ok()
        .assert_contains("você tirou")
        .assert_contains("Ajude quem tirou você");

    // Hints written by Bruno show up for whoever drew him.
    app.post(
        &format!("{bruno}/dicas"),
        &[("wishlist", "Café especial, nada de meia")],
    )
    .await
    .assert_redirect(bruno);
    app.get(bruno).await.assert_contains("Dicas salvas");
    let pairs = reveal_all(&mut app, &links).await;
    let giver_of_bruno = pairs
        .iter()
        .find(|(_, r)| r.as_str() == "Bruno Mello")
        .map(|(g, _)| g.clone())
        .unwrap();
    app.post(&links[&giver_of_bruno], &[])
        .await
        .assert_contains("Café especial, nada de meia");

    // Calendar file.
    let ics = app.get(&format!("{bruno}/evento.ics")).await;
    ics.assert_ok()
        .assert_contains("BEGIN:VCALENDAR")
        .assert_contains("DTSTART;VALUE=DATE:20271224");

    // Organizer dashboard reflects reveals.
    app.clear_cookies();
    app.post("/recuperar", &[("email", "ana@exemplo.com")])
        .await;
    let login = link_in(&app.take_mail().await[0], "/entrar/");
    app.get(&path_of(&login)).await;
    let page = app.get(&base).await;
    page.assert_contains("3<span class=\"of\">/3</span>");
    assert_eq!(page.body.matches("Revelou</span>").count(), 3);
}

#[tokio::test]
async fn adding_someone_after_the_draw_redraws_and_invalidates_old_links() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    let old_links = reveal_links(&app).await;

    add_participant(&mut app, &game_id, "Daniel Mello", "daniel@exemplo.com").await;
    app.get(&base)
        .await
        .assert_contains("sorteio foi refeito")
        .assert_contains("2º sorteio");

    let mail = app.take_mail().await;
    let invites: Vec<_> = mail
        .iter()
        .filter(|m| m.text_body.contains("/revelar/"))
        .collect();
    assert_eq!(invites.len(), 4);
    assert!(
        invites
            .iter()
            .all(|m| m.subject.starts_with("Novo sorteio"))
    );
    let new_links: HashMap<String, String> = invites
        .iter()
        .map(|m| (m.to.clone(), path_of(&link_in(m, "/revelar/"))))
        .collect();

    // Old links are dead, new ones work and form a valid derangement over 4 people.
    for path in old_links.values() {
        assert_eq!(app.get(path).await.status, 404);
    }
    let pairs = reveal_all(&mut app, &new_links).await;
    assert_eq!(pairs.len(), 4);
    assert_valid_derangement(&pairs);
}

#[tokio::test]
async fn removing_someone_after_the_draw_redraws_or_reopens() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 4).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    app.take_mail().await;

    let page = app.get(&base).await;
    let daniel = participant_id(&page.body, "Daniel Mello");
    app.post(&format!("{base}/participantes/{daniel}/remover"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("saiu e o sorteio foi refeito");
    let links = reveal_links(&app).await;
    assert_eq!(links.len(), 3);
    assert!(!links.contains_key("daniel@exemplo.com"));

    // Dropping below the minimum undoes the draw instead.
    let page = app.get(&base).await;
    let carla = participant_id(&page.body, "Carla Souza");
    app.post(&format!("{base}/participantes/{carla}/remover"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("sorteio foi desfeito")
        .assert_contains("Aguardando sorteio");
    // Carla is gone, so her link dies; the others still resolve but show "not drawn yet".
    assert_eq!(app.get(&links["carla@exemplo.com"]).await.status, 404);
    for email in ["ana@exemplo.com", "bruno@exemplo.com"] {
        app.post(&links[email], &[])
            .await
            .assert_redirect_starts(&format!(
                "/revelar/{}",
                links[email].trim_start_matches("/revelar/")
            ));
    }
    assert!(app.take_mail().await.is_empty());
}

#[tokio::test]
async fn resend_rotates_the_link_and_is_throttled() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    let old = reveal_links(&app).await;
    let page = app.get(&base).await;
    let bruno = participant_id(&page.body, "Bruno Mello");

    app.post(&format!("{base}/participantes/{bruno}/reenviar"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Convite reenviado para Bruno Mello");
    let mail = app.take_mail().await;
    assert_eq!(mail.len(), 1);
    assert_eq!(mail[0].to, "bruno@exemplo.com");
    let new_link = path_of(&link_in(&mail[0], "/revelar/"));
    assert_ne!(new_link, old["bruno@exemplo.com"]);
    assert_eq!(app.get(&old["bruno@exemplo.com"]).await.status, 404);
    app.get(&new_link).await.assert_ok();

    // Second resend within ten minutes is refused.
    app.post(&format!("{base}/participantes/{bruno}/reenviar"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base).await.assert_contains("Aguarde 10 minutos");
    assert!(app.take_mail().await.is_empty());

    // "Resend to everyone who hasn't opened" skips those who revealed.
    app.post(&old["carla@exemplo.com"], &[]).await.assert_ok();
    app.post(&format!("{base}/reenviar-pendentes"), &[])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Convites reenviados para 2 pessoas");
    let mail = app.take_mail().await;
    let to: HashSet<String> = mail.iter().map(|m| m.to.clone()).collect();
    assert_eq!(
        to,
        HashSet::from(["ana@exemplo.com".to_owned(), "bruno@exemplo.com".to_owned()])
    );
}

#[tokio::test]
async fn editing_email_after_draw_moves_the_link_and_is_blocked_after_reveal() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/sortear"), &[])
        .await
        .assert_redirect(&base);
    let links = reveal_links(&app).await;
    let page = app.get(&base).await;
    let bruno = participant_id(&page.body, "Bruno Mello");
    let carla = participant_id(&page.body, "Carla Souza");

    app.post(
        &format!("{base}/participantes/editar"),
        &[
            ("pid", &bruno),
            ("name", "Bruno Mello"),
            ("email", "bruno.novo@exemplo.com"),
        ],
    )
    .await
    .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("convite enviado para o novo email");
    let mail = app.take_mail().await;
    assert_eq!(mail.len(), 1);
    assert_eq!(mail[0].to, "bruno.novo@exemplo.com");
    assert_eq!(app.get(&links["bruno@exemplo.com"]).await.status, 404);
    app.get(&path_of(&link_in(&mail[0], "/revelar/")))
        .await
        .assert_ok();

    // Carla reveals, then her email can no longer be changed (name still can).
    app.post(&links["carla@exemplo.com"], &[]).await.assert_ok();
    app.post(
        &format!("{base}/participantes/editar"),
        &[
            ("pid", &carla),
            ("name", "Carla Souza"),
            ("email", "outra@exemplo.com"),
        ],
    )
    .await
    .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("já revelou quem tirou");
    app.post(
        &format!("{base}/participantes/editar"),
        &[
            ("pid", &carla),
            ("name", "Carla S."),
            ("email", "carla@exemplo.com"),
        ],
    )
    .await
    .assert_redirect(&base);
    app.get(&base).await.assert_contains("Carla S.");
    assert!(app.take_mail().await.is_empty());
}

#[tokio::test]
async fn organizer_cannot_see_other_organizers_games() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    app.clear_cookies();
    create_game(&mut app, "Outro", "outra@exemplo.com").await;
    assert_eq!(app.get(&format!("/jogo/{game_id}")).await.status, 404);
    assert_eq!(
        app.post(&format!("/jogo/{game_id}/sortear"), &[])
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn edit_event_message_and_delete() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");

    app.get(&format!("{base}/editar"))
        .await
        .assert_ok()
        .assert_contains("value=\"Natal da família\"");
    app.post(
        &format!("{base}/editar"),
        &[
            ("name", "Natal 2027"),
            ("event_date", "2027-12-25"),
            ("budget", "120"),
            ("organizer_name", "Ana"),
            ("message", "Na casa da vó"),
        ],
    )
    .await
    .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Natal 2027")
        .assert_contains("25 de dezembro de 2027")
        .assert_contains("até R$ 120")
        .assert_contains("Na casa da vó");

    app.post(
        &format!("{base}/mensagem"),
        &[("message", "  Nova mensagem  ")],
    )
    .await
    .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("Nova mensagem")
        .assert_not_contains("Na casa da vó");

    // Delete needs the exact name.
    app.post(&format!("{base}/excluir"), &[("confirm_name", "errado")])
        .await
        .assert_redirect(&base);
    app.get(&base)
        .await
        .assert_contains("digite o nome do evento");
    app.post(
        &format!("{base}/excluir"),
        &[("confirm_name", "natal 2027")],
    )
    .await
    .assert_redirect("/meus");
    assert_eq!(app.get(&base).await.status, 404);
    app.get("/meus")
        .await
        .assert_ok()
        .assert_contains("foi excluído");
}

#[tokio::test]
async fn invitation_email_carries_budget_message_and_reply_to() {
    let mut app = app().await;
    let game_id = game_with_people(&mut app, 3).await;
    let base = format!("/jogo/{game_id}");
    app.post(&format!("{base}/mensagem"), &[("message", "Nada de meia!")])
        .await;
    app.post(&format!("{base}/sortear"), &[]).await;
    let mail = app.take_mail().await;
    let invite = mail.iter().find(|m| m.to == "bruno@exemplo.com").unwrap();
    assert!(invite.subject.contains("Ana convidou você"));
    assert!(invite.text_body.contains("até R$ 80"));
    assert!(invite.text_body.contains("Nada de meia!"));
    assert!(invite.html_body.contains("Revelar meu amigo oculto"));
    assert!(invite.text_body.contains("24 de dezembro de 2027"));
    assert_eq!(invite.reply_to.as_deref(), Some("ana@exemplo.com"));
    let drawn = mail
        .iter()
        .find(|m| m.subject.starts_with("Sorteio realizado"))
        .unwrap();
    assert_eq!(drawn.reply_to, None);
}

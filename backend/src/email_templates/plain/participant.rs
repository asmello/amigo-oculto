use url::Url;

const FOOTER: &str = "---\nAmigo Oculto - Sistema de Sorteio";

/// Participant notification plain-text email
pub fn participant_email(
    participant_name: &str,
    game_name: &str,
    event_date: &str,
    reveal_url: &Url,
) -> String {
    format!(
        "Olá {}!

Você foi convidado para participar do Amigo Oculto \"{}\"!

📅 Data do evento: {}

Para descobrir quem você tirou, acesse o link abaixo:
{}

Guarde este email para consultar seu amigo oculto mais tarde se necessário.

{}",
        participant_name, game_name, event_date, reveal_url, FOOTER
    )
}

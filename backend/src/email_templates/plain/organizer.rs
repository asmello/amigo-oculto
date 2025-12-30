use url::Url;

const FOOTER: &str = "---\nAmigo Oculto - Sistema de Sorteio";

/// Organizer confirmation plain-text email
pub fn organizer_email(
    game_name: &str,
    event_date: &str,
    participant_count: usize,
    admin_url: &Url,
) -> String {
    format!(
        "Parabéns! O sorteio foi realizado com sucesso! 🎉

Amigo Oculto: {}
📅 Data do evento: {}
👥 Participantes: {}

Todos os participantes receberam um email com o link para descobrir quem tiraram.

Para acompanhar quem já visualizou seu amigo oculto, acesse:
{}

⚠️ Importante: Guarde este email para consultar o status do sorteio mais tarde.

{}",
        game_name, event_date, participant_count, admin_url, FOOTER
    )
}
use crate::token::VerificationCode;

const FOOTER: &str = "---\nAmigo Oculto - Sistema de Sorteio";

/// Email verification code plain-text email
pub fn verification_email(game_name: &str, verification_code: VerificationCode) -> String {
    format!(
        "Código de Verificação - Amigo Oculto 🎁

Você está criando o jogo: {}

Seu código de verificação é:

{}

⏱️ Este código expira em 15 minutos.

Digite este código na página de criação do jogo para continuar.

Se você não solicitou este código, ignore este email.

{}",
        game_name, verification_code, FOOTER
    )
}

/// Admin welcome email (sent immediately after game creation)
pub fn admin_welcome_email(game_name: &str, event_date: &str, admin_url: &url::Url) -> String {
    format!(
        "Seu jogo foi criado com sucesso! 🎉

Amigo Oculto: {}
📅 Data do evento: {}

Agora você pode adicionar participantes e realizar o sorteio.

Acesse o painel de administração:
{}

⚠️ Importante: Guarde este link para gerenciar seu jogo. Você precisará dele para:
  • Adicionar participantes
  • Realizar o sorteio
  • Acompanhar quem já visualizou
  • Reenviar emails

{}",
        game_name, event_date, admin_url, FOOTER
    )
}

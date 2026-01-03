use super::base::email_layout;
use crate::email_templates::components::{
    app_footer, gradient_header, info_box, primary_button, warning_box,
};
use crate::token::VerificationCode;
use maud::{Markup, html};
use url::Url;

/// Email verification code template
pub fn verification_email(game_name: &str, verification_code: VerificationCode) -> Markup {
    let content = html! {
        (gradient_header("🔐 Código de Verificação", "Amigo Oculto"))

        div class="content" {
            p { "Você está criando o jogo: " strong { (game_name) } }

            p { "Digite o código abaixo na página de criação para continuar:" }

            div style="text-align: center; margin: 30px 0;" {
                div style="display: inline-block; background: #4A5759; padding: 20px 40px; border-radius: 12px; font-size: 36px; font-weight: bold; color: white; letter-spacing: 8px; font-family: monospace;" {
                    (verification_code)
                }
            }

            (warning_box(html! {
                p {
                    strong { "⏱️ Atenção:" }
                    " Este código expira em 15 minutos."
                }
            }))

            p class="text-muted" {
                "Se você não solicitou este código, ignore este email."
            }
        }

        (app_footer())
    };

    email_layout("Código de Verificação - Amigo Oculto", content)
}

/// Admin welcome email (sent immediately after game creation)
pub fn admin_welcome_email(game_name: &str, event_date: &str, admin_url: &Url) -> Markup {
    let content = html! {
        (gradient_header("🎉 Jogo Criado!", game_name))

        div class="content" {
            p { "Parabéns! Seu jogo foi criado com sucesso!" }

            (info_box(html! {
                p { "📅 " strong { "Data do evento:" } " " (event_date) }
            }))

            p { "Agora você pode:" }
            ul {
                li { "Adicionar participantes" }
                li { "Realizar o sorteio" }
                li { "Acompanhar quem já visualizou" }
                li { "Reenviar emails se necessário" }
            }

            (warning_box(html! {
                p {
                    strong { "⚠️ Importante:" }
                    " Guarde este link! Você precisará dele para gerenciar seu jogo."
                }
            }))

            (primary_button(admin_url, "Acessar Painel de Administração"))

            p class="text-muted" {
                "Se o botão não funcionar, copie e cole este link no seu navegador:"
                br;
                a href=(admin_url.as_str()) { (admin_url.as_str()) }
            }
        }

        (app_footer())
    };

    email_layout(&format!("Jogo Criado - {}", game_name), content)
}

use super::base::email_layout;
use crate::email_templates::components::{app_footer, gradient_header, info_box, primary_button};
use maud::{Markup, html};
use url::Url;

/// Participant notification email template
pub fn participant_email(
    participant_name: &str,
    game_name: &str,
    event_date: &str,
    reveal_url: &Url,
) -> Markup {
    let content = html! {
        (gradient_header("Amigo Oculto", game_name))

        div class="content" {
            p { "Olá " strong { (participant_name) } "!" }

            p {
                "Você foi convidado para participar do Amigo Oculto "
                strong { (game_name) } "!"
            }

            p { "📅 " strong { "Data do evento:" } " " (event_date) }

            (info_box(html! {
                p { "Clique no botão abaixo para descobrir quem você tirou:" }
                (primary_button(reveal_url, "Ver Meu Amigo Oculto"))
            }))

            p class="text-muted" {
                strong { "Dica:" }
                " Guarde este email! Você pode precisar dele para consultar quem você tirou mais tarde."
            }

            p class="text-muted" {
                "Se o botão não funcionar, copie e cole este link no seu navegador:"
                br;
                a href=(reveal_url.as_str()) { (reveal_url.as_str()) }
            }
        }

        (app_footer())
    };

    email_layout(&format!("Amigo Oculto - {}", game_name), content)
}

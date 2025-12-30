use maud::{html, Markup};
use url::Url;
use crate::email_templates::components::{gradient_header, primary_button, success_box, warning_box, app_footer};
use super::base::email_layout;

/// Organizer confirmation email template
pub fn organizer_email(
    game_name: &str,
    event_date: &str,
    participant_count: usize,
    admin_url: &Url,
) -> Markup {
    let content = html! {
        (gradient_header("✅ Sorteio Realizado!", game_name))
        
        div class="content" {
            p { "Parabéns! O sorteio foi realizado com sucesso! 🎉" }
            
            (success_box(html! {
                p { "📅 " strong { "Data do evento:" } " " (event_date) }
                p { "👥 " strong { "Participantes:" } " " (participant_count) }
            }))
            
            p { "Todos os participantes receberam um email com o link para descobrir quem tiraram." }
            
            (warning_box(html! {
                p {
                    strong { "⚠️ Importante:" }
                    " Guarde este email! Use o link abaixo para acompanhar quem já visualizou seu amigo oculto."
                }
            }))
            
            (primary_button(admin_url, "Acompanhar Status"))
            
            p class="text-muted" {
                "Se o botão não funcionar, copie e cole este link no seu navegador:"
                br;
                a href=(admin_url.as_str()) { (admin_url.as_str()) }
            }
        }
        
        (app_footer())
    };
    
    email_layout(&format!("Sorteio Realizado - {}", game_name), content)
}
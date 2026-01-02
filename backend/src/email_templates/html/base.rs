use crate::email_templates::styles::EMAIL_STYLES;
use maud::{DOCTYPE, Markup, PreEscaped, html};

/// Base HTML email layout
pub fn email_layout(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="pt-BR" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) }
                style { (PreEscaped(EMAIL_STYLES)) }
            }
            body {
                (content)
            }
        }
    }
}

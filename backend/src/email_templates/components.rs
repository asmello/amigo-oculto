use maud::{html, Markup};
use url::Url;

/// Header component with gradient background
pub fn gradient_header(title: &str, subtitle: &str) -> Markup {
    html! {
        div class="header" {
            h1 { "🎁 " (title) }
            p { (subtitle) }
        }
    }
}

/// Primary CTA button with gradient
pub fn primary_button(href: &Url, text: &str) -> Markup {
    html! {
        div class="text-center" {
            a href=(href.as_str()) class="btn" {
                (text)
            }
        }
    }
}

/// Info box with content
pub fn info_box(content: Markup) -> Markup {
    html! {
        div class="info-box" {
            (content)
        }
    }
}

/// Success box with content
pub fn success_box(content: Markup) -> Markup {
    html! {
        div class="success-box" {
            (content)
        }
    }
}

/// Warning box with content
pub fn warning_box(content: Markup) -> Markup {
    html! {
        div class="warning-box" {
            (content)
        }
    }
}

/// Footer component
pub fn app_footer() -> Markup {
    html! {
        div class="footer" {
            p { "Amigo Oculto - Sistema de Sorteio" }
        }
    }
}
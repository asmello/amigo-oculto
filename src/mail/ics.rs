//! Minimal iCalendar (RFC 5545) all-day event for the gift exchange.

use chrono::{Datelike, Duration, NaiveDate, Utc};

use crate::fmt;
use crate::models::Game;

pub fn for_game(game: &Game, site_url: &str, domain: &str) -> String {
    let start = game.event_date;
    let end = start + Duration::days(1);
    let mut description = format!("Amigo oculto organizado por {}.", game.organizer_name);
    if let Some(b) = game.budget_cents {
        description.push_str(&format!(" Presente de até {}.", fmt::money(b)));
    }
    if let Some(m) = &game.message {
        description.push_str("\\n\\n");
        description.push_str(&escape(m));
    }
    description.push_str("\\n\\n");
    description.push_str(site_url);

    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let mut out = String::new();
    for line in [
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//Amigo Oculto Simples//PT-BR".to_owned(),
        "CALSCALE:GREGORIAN".to_owned(),
        "METHOD:PUBLISH".to_owned(),
        "BEGIN:VEVENT".to_owned(),
        format!("UID:{}@{domain}", game.id),
        format!("DTSTAMP:{stamp}"),
        format!("DTSTART;VALUE=DATE:{}", ymd(start)),
        format!("DTEND;VALUE=DATE:{}", ymd(end)),
        format!(
            "SUMMARY:{}",
            escape(&format!("Amigo oculto: {}", game.name))
        ),
        format!("DESCRIPTION:{description}"),
        format!("URL:{site_url}"),
        "END:VEVENT".to_owned(),
        "END:VCALENDAR".to_owned(),
    ] {
        fold(&mut out, &line);
    }
    out
}

fn ymd(d: NaiveDate) -> String {
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

/// Escapes text per RFC 5545 §3.3.11.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace('\n', "\\n")
}

/// Content lines are limited to 75 octets; continuation lines start with a space.
fn fold(out: &mut String, line: &str) {
    let mut budget = 75;
    let mut first = true;
    let mut current = String::new();
    for ch in line.chars() {
        let len = ch.len_utf8();
        if current.len() + len > budget {
            out.push_str(&current);
            out.push_str("\r\n");
            current = String::from(" ");
            if first {
                first = false;
                budget = 74;
            }
        }
        current.push(ch);
    }
    out.push_str(&current);
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::GameId;
    use crate::models::GameStatus;

    #[test]
    fn renders_a_valid_looking_calendar() {
        let game = Game {
            id: GameId::new(),
            name: "Natal da família; Mello".into(),
            event_date: NaiveDate::from_ymd_opt(2026, 12, 24).unwrap(),
            budget_cents: Some(8000),
            message: Some("Linha 1\nLinha 2, com vírgula".into()),
            organizer_name: "Ana".into(),
            organizer_email: "ana@exemplo.com".parse().unwrap(),
            status: GameStatus::Drawn,
            draw_round: 1,
            drawn_at: None,
            created_at: 0,
            updated_at: 0,
        };
        let ics = for_game(&game, "https://exemplo.app/", "exemplo.app");
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20261224\r\n"));
        assert!(ics.contains("DTEND;VALUE=DATE:20261225\r\n"));
        assert!(ics.split("\r\n").all(|l| l.len() <= 75));
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
        // Content assertions run on the unfolded text: folding may wrap long lines with CRLF+space.
        let flat = ics.replace("\r\n ", "");
        assert!(flat.contains("SUMMARY:Amigo oculto: Natal da família\\; Mello"));
        assert!(flat.contains("Linha 1\\nLinha 2\\, com vírgula"));
        assert!(flat.contains("Presente de até R$ 80"));
    }
}

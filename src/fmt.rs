//! Brazilian-Portuguese formatting helpers, also exposed as askama filters.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Utc};

const MONTHS: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// Brazil has had no daylight saving time since 2019; UTC-3 covers the family.
pub fn brazil_offset() -> FixedOffset {
    FixedOffset::west_opt(3 * 3600).expect("valid offset")
}

/// Current date in Brazil.
pub fn today() -> NaiveDate {
    Utc::now().with_timezone(&brazil_offset()).date_naive()
}

fn month_name(d: NaiveDate) -> &'static str {
    MONTHS[usize::try_from(d.month0()).expect("month0 < 12")]
}

/// "24 de dezembro de 2026"
pub fn date_long(d: NaiveDate) -> String {
    format!("{} de {} de {}", d.day(), month_name(d), d.year())
}

/// "24 de dezembro"
pub fn date_day_month(d: NaiveDate) -> String {
    format!("{} de {}", d.day(), month_name(d))
}

/// "24/12/2026"
pub fn date_numeric(d: NaiveDate) -> String {
    d.format("%d/%m/%Y").to_string()
}

/// Unix timestamp → "20 de novembro" in Brazil time.
pub fn ts_day_month(ts: i64) -> String {
    match DateTime::<Utc>::from_timestamp(ts, 0) {
        Some(dt) => date_day_month(dt.with_timezone(&brazil_offset()).date_naive()),
        None => String::new(),
    }
}

/// Unix timestamp → "20/11/2026 14:05" in Brazil time.
pub fn ts_numeric(ts: i64) -> String {
    match DateTime::<Utc>::from_timestamp(ts, 0) {
        Some(dt) => dt
            .with_timezone(&brazil_offset())
            .format("%d/%m/%Y %H:%M")
            .to_string(),
        None => String::new(),
    }
}

/// "R$ 80" or "R$ 80,50"
pub fn money(cents: i64) -> String {
    let reais = cents / 100;
    let centavos = cents % 100;
    let mut int = String::new();
    for (i, ch) in reais.to_string().chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            int.push('.');
        }
        int.push(ch);
    }
    let int: String = int.chars().rev().collect();
    if centavos == 0 {
        format!("R$ {int}")
    } else {
        format!("R$ {int},{centavos:02}")
    }
}

/// Parses "80", "80,50", "R$ 1.200,00" into cents.
pub fn parse_money(input: &str) -> Option<i64> {
    let cleaned: String = input
        .trim()
        .trim_start_matches("R$")
        .trim()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Brazilian convention: '.' groups thousands, ',' separates centavos.
    let (int_part, frac_part) = match cleaned.rsplit_once(',') {
        Some((i, f)) => (i.replace('.', ""), f.to_owned()),
        None => (cleaned.replace('.', ""), String::new()),
    };
    let reais: i64 = int_part.parse().ok()?;
    let centavos: i64 = match frac_part.len() {
        0 => 0,
        1 => frac_part.parse::<i64>().ok()? * 10,
        2 => frac_part.parse().ok()?,
        _ => return None,
    };
    Some(reais * 100 + centavos)
}

pub fn first_name(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or(name)
}

/// "Ana Mello" → "AM", "Ana" → "A"
pub fn initials(name: &str) -> String {
    let mut words = name.split_whitespace();
    let first = words.next().and_then(|w| w.chars().next());
    let last = words.last().and_then(|w| w.chars().next());
    first
        .into_iter()
        .chain(last)
        .flat_map(char::to_uppercase)
        .collect()
}

/// One of five palette classes, stable per name, for avatar backgrounds.
pub fn avatar_class(name: &str) -> String {
    let sum: u32 = name.bytes().map(u32::from).sum();
    format!("a{}", sum % 5 + 1)
}

pub mod filters {
    use std::fmt::Display;

    use chrono::NaiveDate;

    #[askama::filter_fn]
    pub fn date_long(d: &NaiveDate, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::date_long(*d))
    }

    #[askama::filter_fn]
    pub fn date_day_month(d: &NaiveDate, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::date_day_month(*d))
    }

    #[askama::filter_fn]
    pub fn date_numeric(d: &NaiveDate, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::date_numeric(*d))
    }

    #[askama::filter_fn]
    pub fn ts_day_month(ts: &i64, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::ts_day_month(*ts))
    }

    #[askama::filter_fn]
    pub fn ts_numeric(ts: &i64, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::ts_numeric(*ts))
    }

    #[askama::filter_fn]
    pub fn money(cents: &i64, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::money(*cents))
    }

    #[askama::filter_fn]
    pub fn first_name(name: impl Display, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::first_name(&name.to_string()).to_owned())
    }

    #[askama::filter_fn]
    pub fn initials(name: impl Display, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::initials(&name.to_string()))
    }

    #[askama::filter_fn]
    pub fn avatar_class(name: impl Display, _: &dyn askama::Values) -> askama::Result<String> {
        Ok(super::avatar_class(&name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        let d = NaiveDate::from_ymd_opt(2026, 12, 24).unwrap();
        assert_eq!(date_long(d), "24 de dezembro de 2026");
        assert_eq!(date_day_month(d), "24 de dezembro");
        assert_eq!(date_numeric(d), "24/12/2026");
    }

    #[test]
    fn money_round_trip() {
        assert_eq!(money(8000), "R$ 80");
        assert_eq!(money(8050), "R$ 80,50");
        assert_eq!(money(120000), "R$ 1.200");
        assert_eq!(parse_money("80"), Some(8000));
        assert_eq!(parse_money("80,5"), Some(8050));
        assert_eq!(parse_money("R$ 1.200,00"), Some(120000));
        assert_eq!(parse_money("abc"), None);
        assert_eq!(parse_money(""), None);
    }

    #[test]
    fn names() {
        assert_eq!(first_name("Ana Clara Mello"), "Ana");
        assert_eq!(initials("Ana Clara Mello"), "AM");
        assert_eq!(initials("ana"), "A");
        assert_eq!(initials("élodie dupont"), "ÉD");
    }
}

//! Validated, normalized email address.

use std::fmt;
use std::str::FromStr;

use lettre::address::Address;
use serde::{Deserialize, Serialize};

/// An email address that parsed successfully and was normalized (trimmed, lowercased).
///
/// Lowercasing the local part is technically lossy, but every real-world provider treats it as
/// case-insensitive and it is what lets `UNIQUE (game_id, email)` catch duplicates.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EmailAddress(String);

impl EmailAddress {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_mailbox(&self, name: Option<&str>) -> lettre::message::Mailbox {
        let address = Address::from_str(&self.0).expect("validated at construction");
        lettre::message::Mailbox::new(name.map(str::to_owned), address)
    }
}

impl fmt::Debug for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmailAddress({})", self.0)
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("endereço de email inválido")]
pub struct InvalidEmail;

impl FromStr for EmailAddress {
    type Err = InvalidEmail;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        if normalized.len() > 254 || !normalized.contains('.') {
            return Err(InvalidEmail);
        }
        Address::from_str(&normalized).map_err(|_| InvalidEmail)?;
        Ok(Self(normalized))
    }
}

impl Serialize for EmailAddress {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for EmailAddress {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl sqlx::Type<sqlx::Sqlite> for EmailAddress {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
    fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for EmailAddress {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        // Stored values were validated on the way in; don't re-validate so a stricter future
        // parser can't make old rows unreadable.
        Ok(Self(s))
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for EmailAddress {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_validates() {
        let e: EmailAddress = "  Ana.Mello@Exemplo.COM ".parse().unwrap();
        assert_eq!(e.as_str(), "ana.mello@exemplo.com");
        assert!("ana@exemplo".parse::<EmailAddress>().is_err());
        assert!("not an email".parse::<EmailAddress>().is_err());
        assert!("".parse::<EmailAddress>().is_err());
    }
}

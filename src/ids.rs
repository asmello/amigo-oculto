//! Identifier and secret newtypes.
//!
//! IDs are ULIDs (sortable, URL-safe). Secrets (reveal links, login links, session ids) are
//! 256-bit random values; only their SHA-256 digest is ever persisted, as [`SecretHash`].

use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ulid::Ulid;

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Ulid);

        impl $name {
            pub fn new() -> Self {
                Self(Ulid::generate())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = ulid::DecodeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Ulid::from_string(s)?))
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Self::from_str(&s).map_err(serde::de::Error::custom)
            }
        }

        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
            fn compatible(ty: &sqlx::sqlite::SqliteTypeInfo) -> bool {
                <String as sqlx::Type<sqlx::Sqlite>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <&str as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                Ok(Self(Ulid::from_string(s)?))
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0.to_string(), args)
            }
        }

        impl askama::filters::HtmlSafe for $name {}
    };
}

define_id!(GameId, "Identifier of a game (sorteio).");
define_id!(ParticipantId, "Identifier of a participant within a game.");
define_id!(
    VerificationId,
    "Identifier of a pending email verification."
);
define_id!(OutboxId, "Identifier of a queued email.");

/// A freshly generated secret (reveal link, login link or session id). Never persisted.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn hash(&self) -> SecretHash {
        SecretHash::of(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Secret {
    type Err = InvalidSecret;

    /// Accepts anything that *looks like* one of our secrets. Lookups go through the hash, so
    /// the only goal here is to reject junk early and bound the input size.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 43
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(InvalidSecret);
        }
        Ok(Self(s.to_owned()))
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid secret format")]
pub struct InvalidSecret;

/// SHA-256 digest (hex) of a [`Secret`]; this is what the database stores.
#[derive(Clone, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct SecretHash(String);

impl SecretHash {
    pub fn of(secret: &str) -> Self {
        let digest = Sha256::digest(secret.as_bytes());
        Self(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretHash({}…)", &self.0[..8])
    }
}

/// Six-digit numeric verification code.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct VerificationCode([u8; 6]);

impl VerificationCode {
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let mut digits = [0u8; 6];
        for d in &mut digits {
            *d = u8::try_from(rng.next_u32() % 10).expect("digit fits in u8");
        }
        Self(digits)
    }

    /// Digest stored in the database (salted with the verification id so equal codes across
    /// verifications don't share a hash).
    pub fn hash(&self, id: VerificationId) -> SecretHash {
        SecretHash::of(&format!("{id}:{self}"))
    }
}

impl fmt::Debug for VerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerificationCode([REDACTED])")
    }
}

impl fmt::Display for VerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for d in self.0 {
            write!(f, "{d}")?;
        }
        Ok(())
    }
}

impl FromStr for VerificationCode {
    type Err = InvalidVerificationCode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits: Vec<u8> = s
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c.to_digit(10).and_then(|d| u8::try_from(d).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or(InvalidVerificationCode)?;
        let arr: [u8; 6] = digits.try_into().map_err(|_| InvalidVerificationCode)?;
        Ok(Self(arr))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("o código deve ter 6 dígitos")]
pub struct InvalidVerificationCode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_unique_and_parse() {
        let a = Secret::generate();
        let b = Secret::generate();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 43);
        assert_eq!(Secret::from_str(a.as_str()).unwrap(), a);
        assert!(Secret::from_str("short").is_err());
        assert!(Secret::from_str(&"x".repeat(43).replace('x', "!")).is_err());
    }

    #[test]
    fn code_round_trips_and_hashes_per_id() {
        let code = VerificationCode::generate();
        let parsed: VerificationCode = code.to_string().parse().unwrap();
        assert_eq!(parsed, code);
        assert_eq!(
            "12 34 56".parse::<VerificationCode>().unwrap().to_string(),
            "123456"
        );
        assert!("12345".parse::<VerificationCode>().is_err());
        assert!("12345a".parse::<VerificationCode>().is_err());
        let id1 = VerificationId::new();
        let id2 = VerificationId::new();
        assert_ne!(code.hash(id1), code.hash(id2));
    }
}

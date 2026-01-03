use lettre::address::Address;
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use ulid::Ulid;

/// Length of secure tokens in characters
const TOKEN_LENGTH: usize = 32;

// =============================================================================
// ID Newtypes (wrap Ulid)
// =============================================================================

macro_rules! define_id_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(Ulid);

        impl $name {
            pub fn new() -> Self {
                Self(Ulid::new())
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
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::from_str(&s).map_err(serde::de::Error::custom)
            }
        }

        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                Ok(Self(Ulid::from_string(&s)?))
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0.to_string(), args)
            }
        }
    };
}

define_id_type!(GameId, "Unique identifier for a Game");
define_id_type!(ParticipantId, "Unique identifier for a Participant");
define_id_type!(VerificationId, "Unique identifier for an EmailVerification");

// =============================================================================
// Token Newtypes (wrap String, distinct types)
// =============================================================================

macro_rules! define_token_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Generate a new secure random token
            pub fn generate() -> Self {
                let token: String = rng()
                    .sample_iter(&Alphanumeric)
                    .take(TOKEN_LENGTH)
                    .map(char::from)
                    .collect();
                Self(token)
            }

            /// Get the token as a string slice
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        // Redacted debug output for security
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}([REDACTED])", stringify!($name))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.to_string()))
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<$name> for String {
            fn from(token: $name) -> String {
                token.0
            }
        }

        impl sqlx::Type<sqlx::Sqlite> for $name {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $name {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                Ok(Self(s))
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for $name {
            fn encode_by_ref(
                &self,
                args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, args)
            }
        }
    };
}

define_token_type!(AdminToken, "Token for game organizer (admin) access");
define_token_type!(ViewToken, "Token for participant match reveal access");
define_token_type!(
    AdminSessionToken,
    "Session token for site administrator access"
);

// =============================================================================
// EmailAddress Newtype (wraps lettre::address::Address for type safety)
// =============================================================================

/// A validated email address.
///
/// This type wraps `lettre::address::Address` to provide:
/// - Validation at parse/deserialization time (fail-fast at API boundaries)
/// - Type safety to prevent mixing email strings with other strings
/// - SQLite persistence support
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EmailAddress(Address);

impl EmailAddress {
    /// Convert to a lettre Mailbox (for sending emails).
    pub fn to_mailbox(&self) -> lettre::message::Mailbox {
        lettre::message::Mailbox::new(None, self.0.clone())
    }
}

impl fmt::Debug for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmailAddress({})", self.0)
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EmailAddress {
    type Err = lettre::address::AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Address::from_str(s)?))
    }
}

impl AsRef<str> for EmailAddress {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Serialize for EmailAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_ref())
    }
}

impl<'de> Deserialize<'de> for EmailAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl sqlx::Type<sqlx::Sqlite> for EmailAddress {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for EmailAddress {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        Ok(Self::from_str(&s)?)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for EmailAddress {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0.to_string(), args)
    }
}

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Length of secure tokens in characters
const TOKEN_LENGTH: usize = 32;

/// A cryptographically secure random token with fixed length
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureToken(String);

impl SecureToken {
    /// Generate a new secure random token
    pub fn generate() -> Self {
        let token: String = thread_rng()
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
    
    /// Get the length of tokens (always TOKEN_LENGTH)
    pub const fn len() -> usize {
        TOKEN_LENGTH
    }
}

impl From<SecureToken> for String {
    fn from(token: SecureToken) -> String {
        token.0
    }
}

impl From<String> for SecureToken {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Display for SecureToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// SQLx integration for Sqlite
impl sqlx::Type<sqlx::Sqlite> for SecureToken {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for SecureToken {
    fn decode(
        value: sqlx::sqlite::SqliteValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        Ok(SecureToken(s))
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for SecureToken {
    fn encode_by_ref(
        &self,
        args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.0, args)
    }
}
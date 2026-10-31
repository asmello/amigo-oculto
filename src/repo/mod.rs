//! Database access, one module per aggregate. Functions take any sqlx executor so they can be
//! used both on the pool and inside a transaction.

pub mod auth;
pub mod exclusions;
pub mod games;
pub mod outbox;
pub mod participants;
pub mod rate;
pub mod verifications;

pub type Exec<'e> = &'e mut sqlx::SqliteConnection;

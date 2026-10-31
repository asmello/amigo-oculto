//! Amigo Oculto Simples — server-rendered Secret Santa for the family.
//!
//! The binary in `main.rs` wires this library to the environment; integration tests build the
//! same [`web::router`] against an in-memory database and a capturing mailer.

pub mod cleanup;
pub mod config;
pub mod db;
pub mod email_addr;
pub mod fmt;
pub mod ids;
pub mod mail;
pub mod matching;
pub mod models;
pub mod repo;
pub mod web;

pub use config::Config;
pub use web::{AppState, router};

//! Phase 03 authentication: PASETO v4 local tokens + argon2id room passwords.

pub mod password;
pub mod room_secret;
pub mod token;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("token: invalid or expired")]
    Token,

    #[error("token: claims mismatch")]
    ClaimsMismatch,

    #[error("password: verification failed")]
    Password,

    #[error("internal: {0}")]
    Internal(String),
}

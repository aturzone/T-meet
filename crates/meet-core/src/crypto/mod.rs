//! Cryptographic primitives used by the server.
//!
//! Submodule layout:
//! - [`passphrase`] — argon2id KDF for the admin passphrase.
//! - [`seal`] — chacha20poly1305 AEAD wrapper (nonce-prepended ciphertext).
//! - [`ca`] — CA certificate generation via `rcgen`.
//! - [`leaf`] — leaf cert issuance with IP / DNS SAN entries.
//! - [`rotation`] — pure rotation policy with an injectable clock.
//!
//! Every primitive uses `OsRng` for entropy. `subtle` powers all secret
//! comparisons. Keys live behind `secrecy::SecretBox` so they cannot land in
//! logs or `Debug` output by accident.

pub mod ca;
pub mod leaf;
pub mod passphrase;
pub mod rotation;
pub mod seal;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("argon2: {0}")]
    Argon2(String),

    #[error("aead encrypt / decrypt failed")]
    Aead,

    #[error("ciphertext blob too short")]
    BlobTooShort,

    #[error("rcgen: {0}")]
    Rcgen(#[from] rcgen::Error),

    #[error("invalid pem")]
    Pem,

    #[error("x509 parse: {0}")]
    X509(String),

    #[error("system time before UNIX epoch")]
    Clock,
}

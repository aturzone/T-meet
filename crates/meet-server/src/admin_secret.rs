//! Per-server admin secret: 32 random bytes, sealed at rest with the
//! admin-passphrase-derived key. Used to mint and verify admin PASETO tokens.

use std::fs;
use std::path::Path;

use meet_core::crypto::seal::{open, seal};
use meet_core::crypto::CryptoError;
use rand::{rngs::OsRng, RngCore};

pub const SECRET_LEN: usize = 32;
const AAD: &[u8] = b"meet-admin-secret/v1";

#[derive(Debug, thiserror::Error)]
pub enum AdminSecretError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),

    #[error("admin secret blob malformed")]
    Malformed,
}

#[must_use]
pub fn generate() -> [u8; SECRET_LEN] {
    let mut s = [0u8; SECRET_LEN];
    OsRng.fill_bytes(&mut s);
    s
}

/// Encrypt + write the secret to `path` with mode 0600.
///
/// # Errors
/// [`AdminSecretError`] on AEAD / io failure.
pub fn write(
    path: &Path,
    secret: &[u8; SECRET_LEN],
    key: &[u8; 32],
) -> Result<(), AdminSecretError> {
    let blob = seal(key, secret, AAD)?;
    crate::init::write_file_0600(path, &blob)?;
    Ok(())
}

/// Read + decrypt the admin secret.
///
/// # Errors
/// [`AdminSecretError`] on AEAD / io failure / wrong-length plaintext.
pub fn read(path: &Path, key: &[u8; 32]) -> Result<[u8; SECRET_LEN], AdminSecretError> {
    let blob = fs::read(path)?;
    let plaintext = open(key, &blob, AAD)?;
    let arr: [u8; SECRET_LEN] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| AdminSecretError::Malformed)?;
    Ok(arr)
}

//! Per-room 32-byte secret used as the PASETO key for that room's tokens.
//!
//! The plaintext secret never leaves memory: it's generated from `OsRng` and
//! immediately sealed with the admin-passphrase-derived key for at-rest
//! storage in `rooms.secret_enc`.

use rand::{rngs::OsRng, RngCore};

use crate::crypto::seal::{open, seal};
use crate::crypto::CryptoError;

pub const SECRET_LEN: usize = 32;
const AAD: &[u8] = b"meet-room-secret/v1";

#[must_use]
pub fn generate() -> [u8; SECRET_LEN] {
    let mut s = [0u8; SECRET_LEN];
    OsRng.fill_bytes(&mut s);
    s
}

/// Encrypt a room secret with the admin-passphrase-derived key.
///
/// # Errors
/// Bubbles up AEAD failure from [`seal`].
pub fn encrypt(secret: &[u8; SECRET_LEN], key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    seal(key, secret, AAD)
}

/// Decrypt a stored room secret blob.
///
/// # Errors
/// [`CryptoError::Aead`] on tag failure; [`CryptoError::BlobTooShort`] when the
/// blob is truncated; sized-array error if the plaintext isn't 32 bytes.
pub fn decrypt(blob: &[u8], key: &[u8; 32]) -> Result<[u8; SECRET_LEN], CryptoError> {
    let plaintext = open(key, blob, AAD)?;
    let arr: [u8; SECRET_LEN] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Aead)?;
    Ok(arr)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_random() {
        let a = generate();
        let b = generate();
        assert_ne!(a, b);
        assert_eq!(a.len(), SECRET_LEN);
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = [7u8; 32];
        let s = generate();
        let blob = encrypt(&s, &key).expect("enc");
        let back = decrypt(&blob, &key).expect("dec");
        assert_eq!(back, s);
    }

    #[test]
    fn wrong_key_rejects() {
        let s = generate();
        let blob = encrypt(&s, &[1u8; 32]).expect("enc");
        assert!(decrypt(&blob, &[2u8; 32]).is_err());
    }
}

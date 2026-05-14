//! `chacha20poly1305` AEAD wrapper.
//!
//! Wire format: `[24-byte nonce] || [ciphertext || 16-byte tag]`.
//!
//! A 4-byte magic + 1-byte version prefix is added by the *blob* layer in
//! [`crate::crypto::ca`] when persisting the CA on disk. This module deals
//! only with the AEAD itself, so it stays self-contained and testable.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::crypto::CryptoError;

/// `XChaCha20` nonce length.
pub const NONCE_LEN: usize = 24;

/// Encrypt `plaintext` under `key`, prepending a fresh random nonce.
///
/// Returns `nonce || ciphertext || tag`.
///
/// # Errors
///
/// Returns [`CryptoError::Aead`] if the underlying AEAD implementation reports
/// a failure (which in practice never happens for chacha20poly1305 with valid
/// inputs).
pub fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Aead)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `blob` (in the `seal` format) under `key`.
///
/// # Errors
///
/// - [`CryptoError::BlobTooShort`] if `blob` is shorter than the nonce.
/// - [`CryptoError::Aead`] on tag verification failure or any other AEAD error.
pub fn open(key: &[u8; 32], blob: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if blob.len() < NONCE_LEN {
        return Err(CryptoError::BlobTooShort);
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce = XNonce::from_slice(nonce_bytes);

    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Aead)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = [9u8; 32];
        let pt = b"hello, t-meet";
        let ct = seal(&key, pt, b"meet-ca/v1").expect("seal");
        assert!(ct.len() > pt.len() + NONCE_LEN);
        let recovered = open(&key, &ct, b"meet-ca/v1").expect("open");
        assert_eq!(recovered, pt);
    }

    #[test]
    fn wrong_key_fails() {
        let ct = seal(&[1u8; 32], b"x", b"").expect("seal");
        assert!(matches!(open(&[2u8; 32], &ct, b""), Err(CryptoError::Aead)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = [1u8; 32];
        let mut ct = seal(&key, b"abc", b"").expect("seal");
        let last = ct.len() - 1;
        ct[last] ^= 0x01;
        assert!(matches!(open(&key, &ct, b""), Err(CryptoError::Aead)));
    }

    #[test]
    fn wrong_aad_fails() {
        let key = [1u8; 32];
        let ct = seal(&key, b"abc", b"context-a").expect("seal");
        assert!(matches!(
            open(&key, &ct, b"context-b"),
            Err(CryptoError::Aead)
        ));
    }

    #[test]
    fn short_blob_rejected() {
        assert!(matches!(
            open(&[0u8; 32], &[0u8; 5], b""),
            Err(CryptoError::BlobTooShort)
        ));
    }

    #[test]
    fn two_seals_produce_distinct_ciphertexts() {
        let key = [3u8; 32];
        let a = seal(&key, b"same", b"").expect("seal");
        let b = seal(&key, b"same", b"").expect("seal");
        assert_ne!(a, b, "nonce should be random per call");
    }
}

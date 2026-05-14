use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretBox};
use zeroize::Zeroize;

use crate::crypto::CryptoError;

/// 16-byte salt; serialized into the encrypted-CA header alongside the
/// ciphertext so that key derivation is reproducible across server restarts.
pub const SALT_LEN: usize = 16;

/// Output key length — fixed at 32 bytes for chacha20poly1305.
pub const KEY_LEN: usize = 32;

/// Argon2id parameters (prompt §4.5 floor):
/// - m = 64 MiB (`65_536` KiB)
/// - t = 3
/// - p = 1
const MEMORY_KIB: u32 = 65_536;
const ITERATIONS: u32 = 3;
const PARALLELISM: u32 = 1;

/// Derive a 32-byte key from `passphrase` and `salt`.
///
/// The output is wrapped in [`secrecy::SecretBox`] so it cannot be accidentally
/// printed via `Debug`. Call [`ExposeSecret::expose_secret`] only at the
/// AEAD boundary.
///
/// # Errors
///
/// Returns [`CryptoError::Argon2`] if the argon2 backend rejects the parameters
/// or fails to compute the hash.
pub fn derive_key(
    passphrase: &SecretBox<String>,
    salt: &[u8; SALT_LEN],
) -> Result<SecretBox<[u8; KEY_LEN]>, CryptoError> {
    let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, Some(KEY_LEN))
        .map_err(|e| CryptoError::Argon2(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.expose_secret().as_bytes(), salt, &mut out)
        .map_err(|e| {
            out.zeroize();
            CryptoError::Argon2(e.to_string())
        })?;

    Ok(SecretBox::new(Box::new(out)))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn pp(s: &str) -> SecretBox<String> {
        SecretBox::new(Box::new(s.to_owned()))
    }

    #[test]
    fn same_inputs_yield_same_key() {
        let salt = [7u8; SALT_LEN];
        let k1 = derive_key(&pp("correct horse"), &salt).expect("kdf");
        let k2 = derive_key(&pp("correct horse"), &salt).expect("kdf");
        assert_eq!(k1.expose_secret(), k2.expose_secret());
    }

    #[test]
    fn different_salt_changes_key() {
        let k1 = derive_key(&pp("hello"), &[1u8; SALT_LEN]).expect("kdf");
        let k2 = derive_key(&pp("hello"), &[2u8; SALT_LEN]).expect("kdf");
        assert_ne!(k1.expose_secret(), k2.expose_secret());
    }

    #[test]
    fn different_passphrase_changes_key() {
        let salt = [0u8; SALT_LEN];
        let k1 = derive_key(&pp("alpha"), &salt).expect("kdf");
        let k2 = derive_key(&pp("beta"), &salt).expect("kdf");
        assert_ne!(k1.expose_secret(), k2.expose_secret());
    }

    #[test]
    fn output_length_is_32() {
        let k = derive_key(&pp("x"), &[0u8; SALT_LEN]).expect("kdf");
        assert_eq!(k.expose_secret().len(), 32);
    }
}

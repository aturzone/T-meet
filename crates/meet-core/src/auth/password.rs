//! argon2id room-password hashing + constant-time verification.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::{Algorithm, Argon2, Params, Version};
use subtle::ConstantTimeEq;

use crate::auth::AuthError;

/// Floor parameters from prompt §4.5: m=64 MiB, t=3, p=1.
fn argon() -> Argon2<'static> {
    // `Params::new` is fallible in the API but the literal arguments here
    // are valid for every supported argon2 version. The default fallback is
    // unreachable in practice.
    let params = Params::new(65_536, 3, 1, None).unwrap_or_default();
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a room password.
///
/// Returns the PHC-string encoding (`$argon2id$v=19$m=...$<salt>$<hash>`).
///
/// # Errors
/// [`AuthError::Internal`] on argon2 failure (does not happen for valid input).
pub fn hash(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::Internal(e.to_string()))?;
    Ok(hash.to_string())
}

/// Constant-time-compare a password against a stored PHC hash.
///
/// # Errors
/// [`AuthError::Password`] on any verification mismatch or parse failure.
pub fn verify(password: &str, encoded: &str) -> Result<(), AuthError> {
    let parsed = PasswordHash::new(encoded).map_err(|_| AuthError::Password)?;
    // argon2's PasswordVerifier::verify_password ALREADY uses constant-time
    // compare internally. We additionally wrap the result through `subtle` so
    // a future swap of the underlying impl can't regress.
    let ok = argon()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();
    let want = 1u8;
    let got = u8::from(ok);
    if want.ct_eq(&got).into() {
        Ok(())
    } else {
        Err(AuthError::Password)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn hash_round_trip() {
        let h = hash("correct horse battery staple").expect("hash");
        verify("correct horse battery staple", &h).expect("verify");
    }

    #[test]
    fn wrong_password_fails() {
        let h = hash("alpha").expect("hash");
        assert!(matches!(verify("beta", &h), Err(AuthError::Password)));
    }

    #[test]
    fn malformed_hash_rejected() {
        assert!(matches!(
            verify("x", "not-a-hash"),
            Err(AuthError::Password)
        ));
    }

    #[test]
    fn two_hashes_of_same_password_differ() {
        let a = hash("alpha").expect("a");
        let b = hash("alpha").expect("b");
        assert_ne!(a, b, "salt must be random per-call");
        // But both verify.
        verify("alpha", &a).expect("a verify");
        verify("alpha", &b).expect("b verify");
    }
}

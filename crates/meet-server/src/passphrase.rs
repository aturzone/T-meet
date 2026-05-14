//! Admin-passphrase intake.
//!
//! Order of preference (prompt §11 contract):
//! 1. `MEET_ADMIN_PASSPHRASE` environment variable.
//! 2. Interactive `rpassword` prompt on a TTY.
//! 3. Refuse to start.
//!
//! The variable is removed from the process environment immediately after
//! reading so it doesn't leak into child processes or `/proc/<pid>/environ`
//! after derivation.

use secrecy::SecretBox;

const ENV_VAR: &str = "MEET_ADMIN_PASSPHRASE";

#[derive(Debug, thiserror::Error)]
pub enum PassphraseError {
    #[error("no passphrase provided — set MEET_ADMIN_PASSPHRASE or run on a TTY")]
    Missing,

    #[error("passphrase too short — at least 8 characters required")]
    TooShort,

    #[error("failed to read passphrase: {0}")]
    Read(String),
}

/// Read the passphrase from env or prompt. Drops the env var after reading.
///
/// # Errors
///
/// See [`PassphraseError`] variants.
pub fn read_admin_passphrase() -> Result<SecretBox<String>, PassphraseError> {
    if let Ok(env_val) = std::env::var(ENV_VAR) {
        // Drop the env var as soon as we have it in `SecretBox` so it
        // doesn't leak into child processes or `/proc/<pid>/environ`.
        std::env::remove_var(ENV_VAR);
        return validate(env_val);
    }

    let prompted = rpassword::prompt_password("Admin passphrase: ")
        .map_err(|e| PassphraseError::Read(e.to_string()))?;

    validate(prompted)
}

fn validate(s: String) -> Result<SecretBox<String>, PassphraseError> {
    if s.is_empty() {
        return Err(PassphraseError::Missing);
    }
    if s.len() < 8 {
        return Err(PassphraseError::TooShort);
    }
    Ok(SecretBox::new(Box::new(s)))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn validate_rejects_empty() {
        assert!(matches!(
            validate(String::new()),
            Err(PassphraseError::Missing)
        ));
    }

    #[test]
    fn validate_rejects_short() {
        assert!(matches!(
            validate("hi".into()),
            Err(PassphraseError::TooShort)
        ));
    }

    #[test]
    fn validate_accepts_long() {
        let pp = validate("correct horse battery staple".into()).expect("ok");
        assert_eq!(pp.expose_secret(), "correct horse battery staple");
    }
}

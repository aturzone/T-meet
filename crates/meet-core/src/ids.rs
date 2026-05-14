//! Opaque IDs: 22-char base64url-of-16-random-bytes for rooms/participants,
//! and a strong random password for server-generated room passwords.

use base64::Engine;
use data_encoding::BASE32_NOPAD;
use rand::{rngs::OsRng, RngCore};

const RAW_ID_BYTES: usize = 16;
const RAW_PASSWORD_BYTES: usize = 16;

/// 22-char base64url ID (no padding). 128 bits of entropy.
#[must_use]
pub fn new_id() -> String {
    let mut buf = [0u8; RAW_ID_BYTES];
    OsRng.fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Strong random password — 26-char base32 string. 128 bits of entropy.
///
/// **Trade-off:** less human-friendly than a wordlist diceware password but
/// avoids baking ~50 KB of EFF wordlist into the binary. The phase doc tracks
/// the wordlist swap as an open question; revisit when there's UX feedback.
#[must_use]
pub fn new_room_password() -> String {
    let mut buf = [0u8; RAW_PASSWORD_BYTES];
    OsRng.fill_bytes(&mut buf);
    BASE32_NOPAD.encode(&buf).to_lowercase()
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn id_has_expected_shape() {
        let id = new_id();
        assert_eq!(id.len(), 22);
        assert!(id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn passwords_are_distinct() {
        let a = new_room_password();
        let b = new_room_password();
        assert_ne!(a, b);
        assert!(a.len() >= 24);
    }
}

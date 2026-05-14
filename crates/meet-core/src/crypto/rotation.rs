//! Leaf-certificate rotation policy.
//!
//! Kept as pure functions with an injectable [`Clock`] so the rotation rules
//! can be unit-tested without `tokio::time::pause` or wall-clock waits.

use time::{Duration, OffsetDateTime};
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::FromDer;

use crate::crypto::CryptoError;

/// Rotate at age >= 60 days (prompt §4.4).
pub const ROTATE_AGE_DAYS: i64 = 60;

pub trait Clock {
    fn now(&self) -> OffsetDateTime;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrozenClock(pub OffsetDateTime);

impl Clock for FrozenClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Reissue,
}

#[derive(Debug, Clone, Copy)]
pub struct LeafInfo {
    pub not_before: OffsetDateTime,
    pub not_after: OffsetDateTime,
}

/// Pull the validity window out of a PEM-encoded leaf cert.
///
/// # Errors
///
/// [`CryptoError::Pem`] on malformed PEM; [`CryptoError::X509`] when the DER
/// payload fails to parse.
pub fn parse_leaf_info(cert_pem: &str) -> Result<LeafInfo, CryptoError> {
    let (_, pem_block) = parse_x509_pem(cert_pem.as_bytes()).map_err(|_| CryptoError::Pem)?;
    let (_, cert) = x509_parser::certificate::X509Certificate::from_der(&pem_block.contents)
        .map_err(|e| CryptoError::X509(e.to_string()))?;
    let nb = OffsetDateTime::from_unix_timestamp(cert.validity().not_before.timestamp())
        .map_err(|_| CryptoError::Clock)?;
    let na = OffsetDateTime::from_unix_timestamp(cert.validity().not_after.timestamp())
        .map_err(|_| CryptoError::Clock)?;
    Ok(LeafInfo {
        not_before: nb,
        not_after: na,
    })
}

#[must_use]
pub fn rotation_action(info: &LeafInfo, clock: &dyn Clock) -> Action {
    let now = clock.now();
    let age = now - info.not_before;
    if age >= Duration::days(ROTATE_AGE_DAYS) || now >= info.not_after {
        Action::Reissue
    } else {
        Action::None
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::crypto::ca::generate_ca;
    use crate::crypto::leaf::{issue_leaf, SanEntry};
    use std::net::{IpAddr, Ipv4Addr};

    fn fresh_leaf(now: OffsetDateTime) -> String {
        let ca = generate_ca("Test CA").expect("ca");
        let leaf = issue_leaf(
            &ca,
            &[SanEntry::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST))],
            "meet.local",
            now,
        )
        .expect("leaf");
        leaf.cert_pem
    }

    #[test]
    fn fresh_cert_does_not_rotate() {
        let now = OffsetDateTime::now_utc();
        let pem = fresh_leaf(now);
        let info = parse_leaf_info(&pem).expect("parse");
        let clock = FrozenClock(now);
        assert_eq!(rotation_action(&info, &clock), Action::None);
    }

    #[test]
    fn rotation_kicks_in_at_60_days() {
        let issued_at = OffsetDateTime::now_utc();
        let pem = fresh_leaf(issued_at);
        let info = parse_leaf_info(&pem).expect("parse");

        let almost = FrozenClock(issued_at + Duration::days(59) + Duration::hours(12));
        assert_eq!(rotation_action(&info, &almost), Action::None);

        let after = FrozenClock(issued_at + Duration::days(60));
        assert_eq!(rotation_action(&info, &after), Action::Reissue);
    }

    #[test]
    fn expired_cert_rotates() {
        let issued_at = OffsetDateTime::now_utc();
        let pem = fresh_leaf(issued_at);
        let info = parse_leaf_info(&pem).expect("parse");
        let after = FrozenClock(issued_at + Duration::days(91));
        assert_eq!(rotation_action(&info, &after), Action::Reissue);
    }
}

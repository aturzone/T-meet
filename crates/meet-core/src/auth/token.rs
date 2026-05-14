//! PASETO v4 local tokens.
//!
//! Two flavours:
//! - **Admin** — sealed with the per-server admin secret. Claims: `iss`, `sub
//!   = "admin"`, `iat`, `exp`.
//! - **Room** — sealed with the per-room secret. Claims: `iss`, `sub =
//!   "room:<id>"`, `iat`, `exp`, plus `pid` (participant id) and `dn`
//!   (display name).
//!
//! TTL caps are hard-coded inside the issuer so callers cannot mint long-lived
//! tokens by passing a large `ttl`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::keys::SymmetricKey;
use pasetors::token::UntrustedToken;
use pasetors::version4::V4;
use pasetors::{local, Local};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::AuthError;

/// Admin tokens: 24h hard cap.
pub const ADMIN_TTL_MAX: Duration = Duration::from_secs(60 * 60 * 24);
/// Room join tokens: 12h hard cap.
pub const ROOM_TTL_MAX: Duration = Duration::from_secs(60 * 60 * 12);

const ISS: &str = "meet-platform";

#[derive(Debug, Clone)]
pub struct AdminClaims {
    pub iat: SystemTime,
    pub exp: SystemTime,
}

#[derive(Debug, Clone)]
pub struct RoomClaims {
    pub room_id: String,
    pub pid: String,
    pub display_name: String,
    pub iat: SystemTime,
    pub exp: SystemTime,
}

/// Mint an admin token.
///
/// # Errors
/// [`AuthError::Internal`] on PASETO library failure (extremely rare).
pub fn issue_admin(secret: &[u8; 32], now: SystemTime, ttl: Duration) -> Result<String, AuthError> {
    let key = SymmetricKey::<V4>::from(secret).map_err(internal)?;
    let ttl = ttl.min(ADMIN_TTL_MAX);
    let claims = base_claims(now, ttl, "admin")?;
    local::encrypt(&key, &claims, None, None).map_err(internal)
}

/// Verify an admin token.
///
/// # Errors
/// [`AuthError::Token`] for parse / decrypt / expiry failures.
pub fn verify_admin(
    secret: &[u8; 32],
    token: &str,
    _now: SystemTime,
) -> Result<AdminClaims, AuthError> {
    let key = SymmetricKey::<V4>::from(secret).map_err(internal)?;
    let untrusted = UntrustedToken::<Local, V4>::try_from(token).map_err(|_| AuthError::Token)?;
    let mut rules = ClaimsValidationRules::new();
    rules.validate_issuer_with(ISS);
    rules.validate_subject_with("admin");
    let trusted =
        local::decrypt(&key, &untrusted, &rules, None, None).map_err(|_| AuthError::Token)?;
    let claims = trusted.payload_claims().ok_or(AuthError::Token)?;
    Ok(AdminClaims {
        iat: parse_ts(claims, "iat")?,
        exp: parse_ts(claims, "exp")?,
    })
}

/// Mint a room join token.
///
/// # Errors
/// [`AuthError::Internal`] on claim construction failure.
pub fn issue_room(
    room_secret: &[u8; 32],
    room_id: &str,
    pid: &str,
    display_name: &str,
    now: SystemTime,
    ttl: Duration,
) -> Result<String, AuthError> {
    let key = SymmetricKey::<V4>::from(room_secret).map_err(internal)?;
    let ttl = ttl.min(ROOM_TTL_MAX);
    let subject = format!("room:{room_id}");
    let mut claims = base_claims(now, ttl, &subject)?;
    claims.add_additional("pid", pid).map_err(internal)?;
    claims
        .add_additional("dn", display_name)
        .map_err(internal)?;
    local::encrypt(&key, &claims, None, None).map_err(internal)
}

/// Verify a room join token against the expected `room_id`.
///
/// # Errors
/// [`AuthError::Token`] for invalid/expired tokens, [`AuthError::ClaimsMismatch`]
/// for room-id mismatch.
pub fn verify_room(
    room_secret: &[u8; 32],
    room_id: &str,
    token: &str,
    _now: SystemTime,
) -> Result<RoomClaims, AuthError> {
    let key = SymmetricKey::<V4>::from(room_secret).map_err(internal)?;
    let untrusted = UntrustedToken::<Local, V4>::try_from(token).map_err(|_| AuthError::Token)?;
    let subject = format!("room:{room_id}");
    let mut rules = ClaimsValidationRules::new();
    rules.validate_issuer_with(ISS);
    rules.validate_subject_with(&subject);
    let trusted =
        local::decrypt(&key, &untrusted, &rules, None, None).map_err(|_| AuthError::Token)?;
    let claims = trusted.payload_claims().ok_or(AuthError::Token)?;

    let pid = claims
        .get_claim("pid")
        .and_then(|v| v.as_str())
        .ok_or(AuthError::ClaimsMismatch)?
        .to_owned();
    let display_name = claims
        .get_claim("dn")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    Ok(RoomClaims {
        room_id: room_id.to_owned(),
        pid,
        display_name,
        iat: parse_ts(claims, "iat")?,
        exp: parse_ts(claims, "exp")?,
    })
}

fn base_claims(now: SystemTime, ttl: Duration, subject: &str) -> Result<Claims, AuthError> {
    let mut c = Claims::new().map_err(internal)?;
    let iat = OffsetDateTime::from(now);
    let exp = OffsetDateTime::from(now + ttl);
    c.issued_at(&iat.format(&Rfc3339).map_err(internal)?)
        .map_err(internal)?;
    c.expiration(&exp.format(&Rfc3339).map_err(internal)?)
        .map_err(internal)?;
    c.issuer(ISS).map_err(internal)?;
    c.subject(subject).map_err(internal)?;
    Ok(c)
}

fn parse_ts(claims: &Claims, name: &str) -> Result<SystemTime, AuthError> {
    let s = claims
        .get_claim(name)
        .and_then(|v| v.as_str())
        .ok_or(AuthError::Token)?;
    let parsed = OffsetDateTime::parse(s, &Rfc3339).map_err(|_| AuthError::Token)?;
    let unix = parsed.unix_timestamp();
    let secs = u64::try_from(unix).map_err(|_| AuthError::Token)?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

fn internal<E: std::fmt::Display>(e: E) -> AuthError {
    AuthError::Internal(e.to_string())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn now() -> SystemTime {
        SystemTime::now()
    }

    #[test]
    fn admin_round_trip() {
        let secret = [9u8; 32];
        let t = issue_admin(&secret, now(), Duration::from_secs(60)).expect("issue");
        let claims = verify_admin(&secret, &t, now()).expect("verify");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn admin_wrong_secret_rejected() {
        let t = issue_admin(&[1u8; 32], now(), Duration::from_secs(60)).expect("issue");
        assert!(matches!(
            verify_admin(&[2u8; 32], &t, now()),
            Err(AuthError::Token)
        ));
    }

    #[test]
    fn admin_ttl_is_capped() {
        let secret = [3u8; 32];
        let n = now();
        let t = issue_admin(&secret, n, Duration::from_secs(60 * 60 * 24 * 7)).expect("issue");
        let claims = verify_admin(&secret, &t, n).expect("verify");
        let actual = claims.exp.duration_since(claims.iat).unwrap_or_default();
        assert!(actual <= ADMIN_TTL_MAX + Duration::from_secs(1));
    }

    #[test]
    fn room_round_trip_with_pid_and_dn() {
        let secret = [7u8; 32];
        let t = issue_room(
            &secret,
            "room-abc",
            "pid-1",
            "Alice",
            now(),
            Duration::from_secs(60),
        )
        .expect("issue");
        let claims = verify_room(&secret, "room-abc", &t, now()).expect("verify");
        assert_eq!(claims.room_id, "room-abc");
        assert_eq!(claims.pid, "pid-1");
        assert_eq!(claims.display_name, "Alice");
    }

    #[test]
    fn room_token_rejects_wrong_room_id() {
        let secret = [7u8; 32];
        let t = issue_room(
            &secret,
            "room-abc",
            "pid-1",
            "Alice",
            now(),
            Duration::from_secs(60),
        )
        .expect("issue");
        assert!(matches!(
            verify_room(&secret, "room-xyz", &t, now()),
            Err(AuthError::Token)
        ));
    }

    #[test]
    fn room_token_rejects_admin_secret() {
        let admin = [1u8; 32];
        let t_admin = issue_admin(&admin, now(), Duration::from_secs(60)).expect("issue");
        assert!(verify_room(&admin, "room-abc", &t_admin, now()).is_err());
    }

    #[test]
    fn tampered_token_rejected() {
        let secret = [9u8; 32];
        let t = issue_admin(&secret, now(), Duration::from_secs(60)).expect("issue");
        let mut bytes: Vec<char> = t.chars().collect();
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == 'a' { 'b' } else { 'a' };
        let tampered: String = bytes.into_iter().collect();
        assert!(verify_admin(&secret, &tampered, now()).is_err());
    }
}

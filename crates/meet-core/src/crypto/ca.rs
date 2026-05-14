//! CA generation + persistence.
//!
//! On-disk blob format (`data/ca.bin`):
//! ```text
//! magic       4 bytes  = b"MTCA"
//! version     1 byte   = 0x01
//! salt        16 bytes
//! sealed      N bytes  = chacha20poly1305 seal of (cert_pem || 0x00 || key_pem)
//!                          with AAD = "meet-ca/v1"
//! ```
//! The plaintext bundles cert + key separated by a single NUL so we don't have
//! to commit to two blobs.

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use secrecy::{ExposeSecret, SecretBox};
use time::{Duration, OffsetDateTime};

use crate::crypto::passphrase::SALT_LEN;
use crate::crypto::seal::{open, seal};
use crate::crypto::CryptoError;

pub const BLOB_MAGIC: &[u8; 4] = b"MTCA";
pub const BLOB_VERSION: u8 = 0x01;
pub const BLOB_AAD: &[u8] = b"meet-ca/v1";

const HEADER_LEN: usize = 4 + 1 + SALT_LEN;

/// CA validity (10 years).
const CA_VALIDITY_DAYS: i64 = 365 * 10;

/// PEM-encoded CA material.
///
/// The key is held in a `SecretBox<String>` so it cannot end up in logs.
pub struct CaMaterial {
    pub cert_pem: String,
    pub key_pem: SecretBox<String>,
}

impl std::fmt::Debug for CaMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaMaterial")
            .field("cert_pem.len", &self.cert_pem.len())
            .field("key_pem", &"<redacted>")
            .finish()
    }
}

/// Generate a fresh self-signed CA with the given common name.
///
/// # Errors
///
/// Bubbles up any [`rcgen`] error.
pub fn generate_ca(common_name: &str) -> Result<CaMaterial, CryptoError> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common_name);
    dn.push(DnType::OrganizationName, "T-meet");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::minutes(5);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    Ok(CaMaterial {
        cert_pem: cert.pem(),
        key_pem: SecretBox::new(Box::new(key_pair.serialize_pem())),
    })
}

/// Pack a `CaMaterial` into an encrypted blob ready for `fs::write`.
///
/// # Errors
///
/// Bubbles up AEAD errors from [`seal`].
pub fn encode_blob(
    ca: &CaMaterial,
    key: &[u8; 32],
    salt: &[u8; SALT_LEN],
) -> Result<Vec<u8>, CryptoError> {
    let mut plaintext =
        Vec::with_capacity(ca.cert_pem.len() + 1 + ca.key_pem.expose_secret().len());
    plaintext.extend_from_slice(ca.cert_pem.as_bytes());
    plaintext.push(0x00);
    plaintext.extend_from_slice(ca.key_pem.expose_secret().as_bytes());

    let sealed = seal(key, &plaintext, BLOB_AAD)?;

    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(BLOB_MAGIC);
    out.push(BLOB_VERSION);
    out.extend_from_slice(salt);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Inspect the on-disk blob header and return the embedded salt without
/// decrypting. Used during startup so we can derive the key before unsealing.
///
/// # Errors
///
/// Returns [`CryptoError::BlobTooShort`] / `Pem` if the blob is malformed.
pub fn read_salt(blob: &[u8]) -> Result<[u8; SALT_LEN], CryptoError> {
    if blob.len() < HEADER_LEN {
        return Err(CryptoError::BlobTooShort);
    }
    if &blob[..4] != BLOB_MAGIC {
        return Err(CryptoError::Pem);
    }
    if blob[4] != BLOB_VERSION {
        return Err(CryptoError::Pem);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&blob[5..5 + SALT_LEN]);
    Ok(salt)
}

/// Decrypt and parse a CA blob.
///
/// # Errors
///
/// Returns AEAD or parse errors if the blob is corrupted or the key is wrong.
pub fn decode_blob(blob: &[u8], key: &[u8; 32]) -> Result<CaMaterial, CryptoError> {
    if blob.len() < HEADER_LEN {
        return Err(CryptoError::BlobTooShort);
    }
    if &blob[..4] != BLOB_MAGIC || blob[4] != BLOB_VERSION {
        return Err(CryptoError::Pem);
    }
    let plaintext = open(key, &blob[HEADER_LEN..], BLOB_AAD)?;
    let sep = plaintext
        .iter()
        .position(|b| *b == 0x00)
        .ok_or(CryptoError::Pem)?;
    let cert_pem = std::str::from_utf8(&plaintext[..sep])
        .map_err(|_| CryptoError::Pem)?
        .to_owned();
    let key_pem = std::str::from_utf8(&plaintext[sep + 1..])
        .map_err(|_| CryptoError::Pem)?
        .to_owned();
    Ok(CaMaterial {
        cert_pem,
        key_pem: SecretBox::new(Box::new(key_pem)),
    })
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn ca_pem_is_parseable() {
        let ca = generate_ca("T-meet test CA").expect("gen");
        assert!(ca.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(
            ca.key_pem
                .expose_secret()
                .starts_with("-----BEGIN PRIVATE KEY-----")
                || ca
                    .key_pem
                    .expose_secret()
                    .starts_with("-----BEGIN PKCS8 PRIVATE KEY-----")
        );
    }

    #[test]
    fn debug_does_not_leak_key() {
        let ca = generate_ca("T-meet test CA").expect("gen");
        let s = format!("{ca:?}");
        assert!(s.contains("redacted"));
        assert!(!s.contains("PRIVATE KEY"));
    }

    #[test]
    fn blob_round_trip() {
        let ca = generate_ca("T-meet test CA").expect("gen");
        let key = [42u8; 32];
        let salt = [7u8; SALT_LEN];
        let blob = encode_blob(&ca, &key, &salt).expect("encode");

        assert_eq!(read_salt(&blob).expect("salt"), salt);

        let decoded = decode_blob(&blob, &key).expect("decode");
        assert_eq!(decoded.cert_pem, ca.cert_pem);
        assert_eq!(decoded.key_pem.expose_secret(), ca.key_pem.expose_secret());
    }

    #[test]
    fn wrong_key_rejects_blob() {
        let ca = generate_ca("T-meet test CA").expect("gen");
        let blob = encode_blob(&ca, &[1u8; 32], &[0u8; SALT_LEN]).expect("encode");
        assert!(decode_blob(&blob, &[2u8; 32]).is_err());
    }

    #[test]
    fn malformed_header_rejected() {
        let mut blob = encode_blob(
            &generate_ca("CN").expect("gen"),
            &[0u8; 32],
            &[0u8; SALT_LEN],
        )
        .expect("encode");
        blob[0] ^= 1;
        assert!(matches!(read_salt(&blob), Err(CryptoError::Pem)));
    }
}

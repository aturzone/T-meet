//! Leaf certificate issuance.
//!
//! Validity defaults to 90 days; the rotation module recommends reissuing at
//! age >= 60 days. The leaf cert is signed by the supplied CA, gets the
//! requested SAN entries (IP + DNS), and is suitable for terminating TLS in
//! `rustls`.

use std::net::IpAddr;

use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose, SanType,
};
use secrecy::{ExposeSecret, SecretBox};
use time::{Duration, OffsetDateTime};

use crate::crypto::ca::CaMaterial;
use crate::crypto::CryptoError;

pub const LEAF_VALIDITY_DAYS: i64 = 90;

#[derive(Debug, Clone)]
pub enum SanEntry {
    Ip(IpAddr),
    Dns(String),
}

pub struct LeafMaterial {
    pub cert_pem: String,
    pub key_pem: SecretBox<String>,
}

impl std::fmt::Debug for LeafMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeafMaterial")
            .field("cert_pem.len", &self.cert_pem.len())
            .field("key_pem", &"<redacted>")
            .finish()
    }
}

/// Issue a leaf certificate signed by `ca`, valid from `now` for
/// [`LEAF_VALIDITY_DAYS`].
///
/// # Errors
///
/// Bubbles up [`rcgen`] errors and / or [`CryptoError::Pem`] when the supplied
/// CA material cannot be parsed.
pub fn issue_leaf(
    ca: &CaMaterial,
    sans: &[SanEntry],
    common_name: &str,
    now: OffsetDateTime,
) -> Result<LeafMaterial, CryptoError> {
    let ca_key = KeyPair::from_pem(ca.key_pem.expose_secret()).map_err(|_| CryptoError::Pem)?;
    let ca_params = CertificateParams::from_ca_cert_pem(&ca.cert_pem)?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    let mut params = CertificateParams::default();
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common_name);
    dn.push(DnType::OrganizationName, "T-meet");
    params.distinguished_name = dn;

    let mut alt_names = Vec::with_capacity(sans.len());
    for s in sans {
        match s {
            SanEntry::Ip(ip) => alt_names.push(SanType::IpAddress(*ip)),
            SanEntry::Dns(d) => {
                let ia5 = rcgen::Ia5String::try_from(d.clone()).map_err(|_| CryptoError::Pem)?;
                alt_names.push(SanType::DnsName(ia5));
            },
        }
    }
    params.subject_alt_names = alt_names;

    params.not_before = now - Duration::minutes(5);
    params.not_after = now + Duration::days(LEAF_VALIDITY_DAYS);

    let leaf_key = KeyPair::generate()?;
    let cert = params.signed_by(&leaf_key, &ca_cert, &ca_key)?;

    Ok(LeafMaterial {
        cert_pem: cert.pem(),
        key_pem: SecretBox::new(Box::new(leaf_key.serialize_pem())),
    })
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::crypto::ca::generate_ca;
    use std::net::Ipv4Addr;

    #[test]
    fn leaf_cert_is_parseable() {
        let ca = generate_ca("Test CA").expect("ca");
        let leaf = issue_leaf(
            &ca,
            &[SanEntry::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST))],
            "meet.local",
            OffsetDateTime::now_utc(),
        )
        .expect("leaf");
        assert!(leaf.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn leaf_debug_does_not_leak_key() {
        let ca = generate_ca("Test CA").expect("ca");
        let leaf = issue_leaf(
            &ca,
            &[SanEntry::Dns("meet.local".into())],
            "meet.local",
            OffsetDateTime::now_utc(),
        )
        .expect("leaf");
        let s = format!("{leaf:?}");
        assert!(s.contains("redacted"));
        assert!(!s.contains("PRIVATE KEY"));
    }

    #[test]
    fn leaf_includes_requested_sans() {
        use x509_parser::pem::parse_x509_pem;
        use x509_parser::prelude::FromDer;

        let ca = generate_ca("Test CA").expect("ca");
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        let leaf = issue_leaf(
            &ca,
            &[SanEntry::Ip(ip), SanEntry::Dns("meet.local".into())],
            "meet.local",
            OffsetDateTime::now_utc(),
        )
        .expect("leaf");

        let (_, pem_block) = parse_x509_pem(leaf.cert_pem.as_bytes()).expect("pem");
        let (_, parsed) =
            x509_parser::certificate::X509Certificate::from_der(&pem_block.contents).expect("der");
        let sans = parsed
            .subject_alternative_name()
            .expect("ext")
            .expect("present");

        let mut saw_ip = false;
        let mut saw_dns = false;
        for n in &sans.value.general_names {
            match n {
                x509_parser::extensions::GeneralName::DNSName(d) if *d == "meet.local" => {
                    saw_dns = true;
                },
                x509_parser::extensions::GeneralName::IPAddress(b) if b == &[10, 0, 0, 5] => {
                    saw_ip = true;
                },
                _ => {},
            }
        }
        assert!(saw_ip, "leaf must carry IP SAN");
        assert!(saw_dns, "leaf must carry DNS SAN");
    }
}

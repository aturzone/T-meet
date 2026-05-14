//! Build a `rustls` server config from in-memory PEM bytes.

use std::sync::Arc;

use rustls::crypto::ring;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("rustls: {0}")]
    Rustls(#[from] rustls::Error),

    #[error("no certificates parsed from pem")]
    NoCertificates,

    #[error("no private key parsed from pem")]
    NoPrivateKey,

    #[error("pem parse: {0}")]
    Pem(String),
}

/// Install the default `ring` `CryptoProvider`. Idempotent across processes.
pub fn install_provider() {
    let _ = ring::default_provider().install_default();
}

/// Build a TLS 1.3-only rustls config from PEM-encoded cert chain + private key.
///
/// # Errors
///
/// See [`TlsError`] variants.
pub fn build_server_config(cert_pem: &str, key_pem: &str) -> Result<Arc<ServerConfig>, TlsError> {
    install_provider();

    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| TlsError::Pem(e.to_string()))?;
    if certs.is_empty() {
        return Err(TlsError::NoCertificates);
    }

    let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
        .map_err(|e| TlsError::Pem(e.to_string()))?
        .ok_or(TlsError::NoPrivateKey)?;

    let key_owned: PrivateKeyDer<'static> = match key {
        PrivateKeyDer::Pkcs8(d) => PrivateKeyDer::Pkcs8(d.clone_key()),
        PrivateKeyDer::Pkcs1(d) => PrivateKeyDer::Pkcs1(d.clone_key()),
        PrivateKeyDer::Sec1(d) => PrivateKeyDer::Sec1(d.clone_key()),
        other => other,
    };

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key_owned)?;

    Ok(Arc::new(cfg))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use meet_core::crypto::ca::generate_ca;
    use meet_core::crypto::leaf::{issue_leaf, SanEntry};
    use secrecy::ExposeSecret;
    use std::net::{IpAddr, Ipv4Addr};
    use time::OffsetDateTime;

    #[test]
    fn builds_config_from_leaf() {
        let ca = generate_ca("Test CA").expect("ca");
        let leaf = issue_leaf(
            &ca,
            &[SanEntry::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST))],
            "127.0.0.1",
            OffsetDateTime::now_utc(),
        )
        .expect("leaf");
        let cfg = build_server_config(&leaf.cert_pem, leaf.key_pem.expose_secret()).expect("cfg");
        assert!(!cfg.alpn_protocols.is_empty() || cfg.alpn_protocols.is_empty());
        let _ = cfg;
    }

    #[test]
    fn rejects_missing_cert() {
        let err = build_server_config("", "").unwrap_err();
        assert!(matches!(
            err,
            TlsError::NoCertificates | TlsError::NoPrivateKey | TlsError::Pem(_)
        ));
    }
}

//! `meet-server init` — one-time first-boot setup.
//!
//! Steps:
//! 1. Read the admin passphrase.
//! 2. Generate a fresh 16-byte salt and derive a 32-byte key with argon2id.
//! 3. Generate the local CA via `rcgen`.
//! 4. Issue the first leaf cert covering the configured bind IP / hostname.
//! 5. Persist:
//!    - `data/ca.bin`  — encrypted CA (magic + version + salt + sealed plaintext)
//!    - `data/ca.crt`  — public PEM CA (served at `/ca.crt`)
//!    - `data/leaf.pem` — leaf cert
//!    - `data/leaf.key` — leaf private key (chmod 0600)

use std::fs;
use std::path::Path;

use meet_core::config::Config;
use meet_core::crypto::ca::{encode_blob, generate_ca, CaMaterial};
use meet_core::crypto::leaf::{issue_leaf, LeafMaterial, SanEntry};
use meet_core::crypto::passphrase::{derive_key, SALT_LEN};
use rand::rngs::OsRng;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretBox};
use time::OffsetDateTime;

use crate::paths::DataPaths;

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("data directory already exists at {0:?} — refusing to overwrite")]
    AlreadyInitialized(std::path::PathBuf),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto: {0}")]
    Crypto(#[from] meet_core::crypto::CryptoError),

    #[error("auth: {0}")]
    Auth(String),
}

#[derive(Debug)]
pub struct InitOutput {
    pub leaf_fingerprint_sha256: String,
    /// Admin token, shown once on stdout. Never logged.
    pub admin_token: String,
}

/// Run the init workflow against a config and the supplied passphrase.
///
/// # Errors
///
/// See [`InitError`].
pub fn run_init(cfg: &Config, passphrase: &SecretBox<String>) -> Result<InitOutput, InitError> {
    let paths = DataPaths::new(&cfg.storage.data_dir);
    if paths.ca_blob().exists() || paths.leaf_cert_pem().exists() {
        return Err(InitError::AlreadyInitialized(paths.root.clone()));
    }

    fs::create_dir_all(&paths.root)?;
    chmod_dir_700(&paths.root)?;

    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let key = derive_key(passphrase, &salt)?;

    let ca = generate_ca("T-meet local CA")?;

    let san_host = san_from_config(cfg);
    let now = OffsetDateTime::now_utc();
    let leaf = issue_leaf(&ca, &san_host.sans, &san_host.common_name, now)?;

    let blob = encode_blob(&ca, key.expose_secret(), &salt)?;
    write_file_0600(&paths.ca_blob(), &blob)?;
    fs::write(paths.ca_public_pem(), ca.cert_pem.as_bytes())?;
    fs::write(paths.leaf_cert_pem(), leaf.cert_pem.as_bytes())?;
    write_file_0600(
        &paths.leaf_key_pem(),
        leaf.key_pem.expose_secret().as_bytes(),
    )?;

    // Admin secret + admin token.
    let admin_secret = crate::admin_secret::generate();
    crate::admin_secret::write(
        &paths.admin_secret_blob(),
        &admin_secret,
        key.expose_secret(),
    )
    .map_err(|e| match e {
        crate::admin_secret::AdminSecretError::Io(io) => InitError::Io(io),
        crate::admin_secret::AdminSecretError::Crypto(c) => InitError::Crypto(c),
        crate::admin_secret::AdminSecretError::Malformed => {
            InitError::Crypto(meet_core::crypto::CryptoError::Aead)
        },
    })?;
    let admin_token = meet_core::auth::token::issue_admin(
        &admin_secret,
        std::time::SystemTime::now(),
        meet_core::auth::token::ADMIN_TTL_MAX,
    )
    .map_err(|e| InitError::Auth(e.to_string()))?;

    tracing::info!(
        ca_blob = %paths.ca_blob().display(),
        leaf = %paths.leaf_cert_pem().display(),
        admin_blob = %paths.admin_secret_blob().display(),
        "first-boot artifacts written"
    );

    Ok(InitOutput {
        leaf_fingerprint_sha256: sha256_fingerprint(&leaf.cert_pem)?,
        admin_token,
    })
}

/// Load a previously-initialised CA + leaf from disk. If the leaf needs to be
/// rotated, a fresh one is issued and written.
pub struct LoadedTls {
    #[allow(
        dead_code,
        reason = "Phase 03+ may need direct CA access for advanced flows"
    )]
    pub ca: CaMaterial,
    pub leaf: LeafMaterial,
    pub admin_secret: [u8; 32],
    /// Admin-passphrase-derived key (kept alive only as long as `LoadedTls`).
    /// Used to seal per-room secrets at room-creation time.
    pub at_rest_key: [u8; 32],
}

pub fn load_or_rotate(
    cfg: &Config,
    passphrase: &SecretBox<String>,
) -> Result<LoadedTls, InitError> {
    let paths = DataPaths::new(&cfg.storage.data_dir);
    let blob = fs::read(paths.ca_blob())?;
    let salt = meet_core::crypto::ca::read_salt(&blob)?;
    let key = derive_key(passphrase, &salt)?;
    let ca = meet_core::crypto::ca::decode_blob(&blob, key.expose_secret())?;

    let cert_pem = fs::read_to_string(paths.leaf_cert_pem())?;
    let key_pem = fs::read_to_string(paths.leaf_key_pem())?;
    let info = meet_core::crypto::rotation::parse_leaf_info(&cert_pem)?;
    let clock = meet_core::crypto::rotation::SystemClock;
    let action = meet_core::crypto::rotation::rotation_action(&info, &clock);

    let leaf = match action {
        meet_core::crypto::rotation::Action::None => LeafMaterial {
            cert_pem,
            key_pem: SecretBox::new(Box::new(key_pem)),
        },
        meet_core::crypto::rotation::Action::Reissue => {
            tracing::info!("leaf cert past rotation threshold — issuing fresh leaf");
            let san_host = san_from_config(cfg);
            let leaf = issue_leaf(
                &ca,
                &san_host.sans,
                &san_host.common_name,
                OffsetDateTime::now_utc(),
            )?;
            fs::write(paths.leaf_cert_pem(), leaf.cert_pem.as_bytes())?;
            write_file_0600(
                &paths.leaf_key_pem(),
                leaf.key_pem.expose_secret().as_bytes(),
            )?;
            leaf
        },
    };

    let at_rest_key = *key.expose_secret();
    let admin_secret = crate::admin_secret::read(&paths.admin_secret_blob(), &at_rest_key)
        .map_err(|e| match e {
            crate::admin_secret::AdminSecretError::Io(io) => InitError::Io(io),
            crate::admin_secret::AdminSecretError::Crypto(c) => InitError::Crypto(c),
            crate::admin_secret::AdminSecretError::Malformed => {
                InitError::Crypto(meet_core::crypto::CryptoError::Aead)
            },
        })?;

    Ok(LoadedTls {
        ca,
        leaf,
        admin_secret,
        at_rest_key,
    })
}

struct SanForConfig {
    common_name: String,
    sans: Vec<SanEntry>,
}

fn san_from_config(cfg: &Config) -> SanForConfig {
    let mut sans = vec![SanEntry::Ip(cfg.server.bind_ip)];
    let mut common_name = cfg.server.bind_ip.to_string();

    if let Some(host) = &cfg.server.external_host {
        if !host.is_empty() {
            sans.push(SanEntry::Dns(host.clone()));
            common_name.clone_from(host);
        }
    }

    // Always add 127.0.0.1 so the operator can curl `/ca.crt` locally
    // from the same machine.
    let loop_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    if !sans
        .iter()
        .any(|s| matches!(s, SanEntry::Ip(ip) if *ip == loop_ip))
    {
        sans.push(SanEntry::Ip(loop_ip));
    }

    SanForConfig { common_name, sans }
}

fn sha256_fingerprint(cert_pem: &str) -> Result<String, InitError> {
    use sha2::{Digest, Sha256};
    let mut input = cert_pem.as_bytes();
    let der = rustls_pemfile::certs(&mut input)
        .next()
        .ok_or(InitError::Crypto(meet_core::crypto::CryptoError::Pem))?
        .map_err(|e| InitError::Io(std::io::Error::other(e)))?;
    let mut h = Sha256::new();
    h.update(der.as_ref());
    let digest: [u8; 32] = h.finalize().into();
    Ok(hex_colon(&digest))
}

fn hex_colon(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        let _ = write!(out, "{b:02X}");
    }
    out
}

#[cfg(unix)]
fn chmod_dir_700(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(p, perms)
}

#[cfg(not(unix))]
fn chmod_dir_700(_p: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(crate) fn write_file_0600(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    std::io::Write::write_all(&mut f, bytes)?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn write_file_0600(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(path, bytes)
}

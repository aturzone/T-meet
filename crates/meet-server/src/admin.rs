//! `meet-server admin …` and `meet-server doctor` subcommands.
//!
//! These run outside the serve event loop and only need the at-rest key, the
//! data dir, and (for status) the `SQLite` file. None of them touch the
//! network.

use std::fs;
use std::time::SystemTime;

use meet_core::config::Config;
use meet_core::crypto::passphrase::{derive_key, SALT_LEN};
use meet_core::crypto::rotation::parse_leaf_info;
use secrecy::{ExposeSecret, SecretBox};
use time::OffsetDateTime;

use crate::admin_secret;
use crate::init::write_file_0600;
use crate::paths::DataPaths;

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("data dir not initialized — run `meet-server init` first")]
    NotInitialized,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto: {0}")]
    Crypto(#[from] meet_core::crypto::CryptoError),

    #[error("auth: {0}")]
    Auth(String),

    #[error("db: {0}")]
    Db(String),
}

/// Decrypt the on-disk CA blob to recover the at-rest key. Used by every
/// `admin` subcommand so they can re-seal updated material.
fn decrypt_at_rest_key(
    paths: &DataPaths,
    passphrase: &SecretBox<String>,
) -> Result<[u8; 32], AdminError> {
    let blob_path = paths.ca_blob();
    if !blob_path.exists() {
        return Err(AdminError::NotInitialized);
    }
    let blob = fs::read(&blob_path)?;
    let salt: [u8; SALT_LEN] = meet_core::crypto::ca::read_salt(&blob)?;
    let key = derive_key(passphrase, &salt)?;
    // Verify by attempting decryption — wrong passphrase fails here.
    meet_core::crypto::ca::decode_blob(&blob, key.expose_secret())?;
    Ok(*key.expose_secret())
}

/// `meet-server admin token regenerate` — rotate the admin secret + mint and
/// print a fresh admin token.
pub fn regenerate_admin_token(
    cfg: &Config,
    passphrase: &SecretBox<String>,
) -> Result<String, AdminError> {
    let paths = DataPaths::new(&cfg.storage.data_dir);
    let key = decrypt_at_rest_key(&paths, passphrase)?;

    let new_secret = admin_secret::generate();
    let blob = meet_core::crypto::seal::seal(&key, &new_secret, b"meet-admin-secret/v1")?;
    write_file_0600(&paths.admin_secret_blob(), &blob)?;

    let token = meet_core::auth::token::issue_admin(
        &new_secret,
        SystemTime::now(),
        meet_core::auth::token::ADMIN_TTL_MAX,
    )
    .map_err(|e| AdminError::Auth(e.to_string()))?;

    Ok(token)
}

/// Non-sensitive status snapshot.
#[derive(Debug)]
pub struct Status {
    pub data_dir: std::path::PathBuf,
    pub leaf_not_before: OffsetDateTime,
    pub leaf_not_after: OffsetDateTime,
    pub leaf_days_remaining: i64,
    pub rooms: i64,
    pub audit_entries: i64,
    pub db_size_bytes: u64,
}

/// `meet-server admin status` — print non-sensitive operational state. No
/// passphrase needed (everything read here is already operator-visible).
pub fn status(cfg: &Config) -> Result<Status, AdminError> {
    let paths = DataPaths::new(&cfg.storage.data_dir);
    if !paths.leaf_cert_pem().exists() {
        return Err(AdminError::NotInitialized);
    }
    let pem = fs::read_to_string(paths.leaf_cert_pem())?;
    let info = parse_leaf_info(&pem)?;
    let now = OffsetDateTime::now_utc();
    let days_remaining = (info.not_after - now).whole_days();

    let db_path = paths.db_file();
    let db_size_bytes = fs::metadata(&db_path).map_or(0, |m| m.len());

    let (rooms, audit_entries) = if db_path.exists() {
        count_rows_blocking(&db_path).unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    Ok(Status {
        data_dir: paths.root.clone(),
        leaf_not_before: info.not_before,
        leaf_not_after: info.not_after,
        leaf_days_remaining: days_remaining,
        rooms,
        audit_entries,
        db_size_bytes,
    })
}

fn count_rows_blocking(db_path: &std::path::Path) -> Result<(i64, i64), AdminError> {
    use std::sync::mpsc;
    let path = db_path.to_owned();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = (|| -> Result<(i64, i64), AdminError> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| AdminError::Db(e.to_string()))?;
            rt.block_on(async {
                let db = meet_core::db::Db::open(&path)
                    .await
                    .map_err(|e| AdminError::Db(e.to_string()))?;
                let rooms: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rooms")
                    .fetch_one(&db.pool)
                    .await
                    .map_err(|e| AdminError::Db(e.to_string()))?;
                let audit: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_log")
                    .fetch_one(&db.pool)
                    .await
                    .map_err(|e| AdminError::Db(e.to_string()))?;
                Ok((rooms.0, audit.0))
            })
        })();
        let _ = tx.send(res);
    });
    rx.recv().map_err(|e| AdminError::Db(e.to_string()))?
}

#[derive(Debug)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

/// `meet-server doctor` — read-only pre-flight. Verifies file permissions,
/// data dir integrity, port availability.
#[must_use]
pub fn doctor(cfg: &Config) -> DoctorReport {
    let mut checks = Vec::new();
    let paths = DataPaths::new(&cfg.storage.data_dir);

    checks.push(check_file_exists(
        "data/ca.bin",
        &paths.ca_blob(),
        "run `meet-server init` first",
    ));
    checks.push(check_file_exists(
        "data/admin.bin",
        &paths.admin_secret_blob(),
        "admin secret missing — re-init or restore from backup",
    ));
    checks.push(check_file_exists(
        "data/leaf.pem",
        &paths.leaf_cert_pem(),
        "leaf cert missing — re-init",
    ));
    checks.push(check_file_exists(
        "data/leaf.key",
        &paths.leaf_key_pem(),
        "leaf key missing — re-init",
    ));

    checks.push(check_mode_0600(
        "data/admin.bin",
        &paths.admin_secret_blob(),
    ));
    checks.push(check_mode_0600("data/ca.bin", &paths.ca_blob()));
    checks.push(check_mode_0600("data/leaf.key", &paths.leaf_key_pem()));

    if paths.db_file().exists() {
        checks.push(check_mode_0600("data/meet.db", &paths.db_file()));
    }

    checks.push(check_port(
        "tls_port",
        cfg.server.bind_ip,
        cfg.server.tls_port,
    ));
    checks.push(check_port(
        "http_redirect_port",
        cfg.server.bind_ip,
        cfg.server.http_redirect_port,
    ));

    DoctorReport { checks }
}

fn check_file_exists(name: &str, p: &std::path::Path, hint: &str) -> DoctorCheck {
    if p.exists() {
        DoctorCheck {
            name: name.to_owned(),
            status: DoctorStatus::Ok,
            detail: format!("present at {}", p.display()),
        }
    } else {
        DoctorCheck {
            name: name.to_owned(),
            status: DoctorStatus::Fail,
            detail: hint.to_owned(),
        }
    }
}

#[cfg(unix)]
fn check_mode_0600(name: &str, p: &std::path::Path) -> DoctorCheck {
    use std::os::unix::fs::PermissionsExt;
    if !p.exists() {
        return DoctorCheck {
            name: name.to_owned(),
            status: DoctorStatus::Warn,
            detail: "missing — skipping mode check".into(),
        };
    }
    match fs::metadata(p) {
        Ok(m) => {
            let mode = m.permissions().mode() & 0o777;
            if mode == 0o600 {
                DoctorCheck {
                    name: name.to_owned(),
                    status: DoctorStatus::Ok,
                    detail: "mode 0600".into(),
                }
            } else {
                DoctorCheck {
                    name: name.to_owned(),
                    status: DoctorStatus::Warn,
                    detail: format!("mode {mode:#o}, expected 0600"),
                }
            }
        },
        Err(e) => DoctorCheck {
            name: name.to_owned(),
            status: DoctorStatus::Warn,
            detail: format!("stat failed: {e}"),
        },
    }
}

#[cfg(not(unix))]
fn check_mode_0600(name: &str, _p: &std::path::Path) -> DoctorCheck {
    DoctorCheck {
        name: name.to_owned(),
        status: DoctorStatus::Ok,
        detail: "mode check skipped on non-unix".into(),
    }
}

fn check_port(name: &str, bind: std::net::IpAddr, port: u16) -> DoctorCheck {
    let addr = std::net::SocketAddr::new(bind, port);
    match std::net::TcpListener::bind(addr) {
        Ok(l) => {
            drop(l);
            DoctorCheck {
                name: name.to_owned(),
                status: DoctorStatus::Ok,
                detail: format!("{addr} bindable"),
            }
        },
        Err(e) => DoctorCheck {
            name: name.to_owned(),
            status: DoctorStatus::Fail,
            detail: format!("{addr}: {e}"),
        },
    }
}

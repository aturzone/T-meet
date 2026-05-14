//! `meet-server serve` — load the CA + leaf and run an HTTPS listener.
//!
//! Phase 01 surface: just `GET /healthz` and `GET /ca.crt`. Phase 02 grows the
//! router; the construction shape here stays.

use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use meet_core::config::Config;
use secrecy::{ExposeSecret, SecretBox};

use crate::init::{load_or_rotate, InitError};
use crate::paths::DataPaths;
use crate::routes::{ca, ca::CaSource, health};
use crate::tls::{build_server_config, TlsError};

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("init: {0}")]
    Init(#[from] InitError),

    #[error("tls: {0}")]
    Tls(#[from] TlsError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn run_serve(cfg: Config, passphrase: SecretBox<String>) -> Result<(), ServeError> {
    let paths = DataPaths::new(&cfg.storage.data_dir);
    let loaded = load_or_rotate(&cfg, &passphrase)?;
    drop(passphrase);

    let server_cfg =
        build_server_config(&loaded.leaf.cert_pem, loaded.leaf.key_pem.expose_secret())?;
    let rustls_cfg = RustlsConfig::from_config(server_cfg);

    let app: Router = Router::new().route("/healthz", get(health::handler)).route(
        "/ca.crt",
        get(ca::handler).with_state(CaSource {
            path: paths.ca_public_pem(),
        }),
    );

    let addr = SocketAddr::new(cfg.server.bind_ip, cfg.server.tls_port);
    tracing::info!(
        bind = %addr,
        http_redirect_port = cfg.server.http_redirect_port,
        "serving https"
    );

    axum_server::bind_rustls(addr, rustls_cfg)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

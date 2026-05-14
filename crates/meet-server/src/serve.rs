//! `meet-server serve` — load the CA + leaf, open the DB, run two listeners:
//! - HTTPS (axum-server + rustls) — the real surface.
//! - Plain-HTTP — 301s everything to HTTPS, plus serves `/ca.crt` so first-time
//!   trust works on devices that don't yet trust the CA.

use std::net::SocketAddr;
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use meet_core::config::Config;
use meet_core::db::Db;
use secrecy::{ExposeSecret, SecretBox};

use crate::app::{build_app, build_redirect_app, AppState};
use crate::init::{load_or_rotate, InitError};
use crate::middleware::rate_limit::RateLimiter;
use crate::paths::DataPaths;
use crate::tls::{build_server_config, TlsError};

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("init: {0}")]
    Init(#[from] InitError),

    #[error("tls: {0}")]
    Tls(#[from] TlsError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("db: {0}")]
    Db(#[from] meet_core::db::DbError),
}

pub async fn run_serve(cfg: Config, passphrase: SecretBox<String>) -> Result<(), ServeError> {
    let paths = Arc::new(DataPaths::new(&cfg.storage.data_dir));
    let loaded = load_or_rotate(&cfg, &passphrase)?;
    drop(passphrase);

    let db_path = paths.root.join("meet.db");
    let db = Arc::new(Db::open(&db_path).await?);
    tracing::info!(db = %db_path.display(), "database open");

    let server_cfg =
        build_server_config(&loaded.leaf.cert_pem, loaded.leaf.key_pem.expose_secret())?;
    let rustls_cfg = RustlsConfig::from_config(server_cfg);

    let state = AppState {
        db: db.clone(),
        paths: paths.clone(),
        admin_secret: Arc::new(loaded.admin_secret),
        at_rest_key: Arc::new(loaded.at_rest_key),
        rate_limiter: Arc::new(RateLimiter::new()),
        room_hub: crate::signaling::room_hub::shared_hub(),
        sfu: Arc::new(
            meet_sfu::Sfu::new_default().map_err(|e| std::io::Error::other(e.to_string()))?,
        ),
        bind_ip: cfg.server.bind_ip,
        external_host: cfg.server.external_host.clone(),
    };
    let app = build_app(state);

    let tls_listener_addr = SocketAddr::new(cfg.server.bind_ip, cfg.server.tls_port);
    let redirect_listener_addr = SocketAddr::new(cfg.server.bind_ip, cfg.server.http_redirect_port);

    let host = cfg
        .server
        .external_host
        .clone()
        .unwrap_or_else(|| cfg.server.bind_ip.to_string());

    let redirect_app = build_redirect_app(host.clone(), cfg.server.tls_port, paths.ca_public_pem());

    tracing::info!(
        https = %tls_listener_addr,
        http_redirect = %redirect_listener_addr,
        external_host = %host,
        "listeners up"
    );

    let https = tokio::spawn(async move {
        axum_server::bind_rustls(tls_listener_addr, rustls_cfg)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
    });

    let http = tokio::spawn(async move {
        axum_server::bind(redirect_listener_addr)
            .serve(redirect_app.into_make_service())
            .await
    });

    tokio::select! {
        r = https => r.map_err(std::io::Error::other)??,
        r = http  => r.map_err(std::io::Error::other)??,
    }

    Ok(())
}

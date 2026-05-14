//! `GET /api/setup-info` — public, returns the leaf cert fingerprint so the
//! `/setup` page can show a verifiable hash.

use std::fmt::Write;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::app::AppState;

#[derive(Serialize)]
pub struct SetupInfo {
    pub leaf_fingerprint_sha256: String,
    pub ca_cert_url: String,
}

pub async fn handler(
    State(state): State<AppState>,
) -> Result<Json<SetupInfo>, (StatusCode, &'static str)> {
    let Ok(cert_pem) = tokio::fs::read_to_string(state.paths.leaf_cert_pem()).await else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "leaf unavailable"));
    };
    let fingerprint = sha256_fingerprint(&cert_pem)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "leaf parse failed"))?;
    Ok(Json(SetupInfo {
        leaf_fingerprint_sha256: fingerprint,
        ca_cert_url: "/ca.crt".to_owned(),
    }))
}

fn sha256_fingerprint(cert_pem: &str) -> Option<String> {
    let mut input = cert_pem.as_bytes();
    let der = rustls_pemfile::certs(&mut input).next()?.ok()?;
    let mut h = Sha256::new();
    h.update(der.as_ref());
    let digest: [u8; 32] = h.finalize().into();
    let mut out = String::with_capacity(digest.len() * 3);
    for (i, b) in digest.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        let _ = write!(out, "{b:02X}");
    }
    Some(out)
}

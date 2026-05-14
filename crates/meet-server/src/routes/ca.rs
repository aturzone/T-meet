//! `GET /ca.crt` — serve the local CA certificate so users can trust it once.

use std::path::PathBuf;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

#[derive(Debug, Clone)]
pub struct CaSource {
    pub path: PathBuf,
}

pub async fn handler(State(src): State<CaSource>) -> Response {
    match tokio::fs::read(&src.path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/x-pem-file"),
                (header::CACHE_CONTROL, "no-store"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"meet-ca.crt\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, path = %src.path.display(), "ca.crt read failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "ca cert unavailable").into_response()
        },
    }
}

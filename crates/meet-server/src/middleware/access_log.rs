//! Structured access logs. Info level: method, path, status, latency, `request_id`.
//! No IP at info level (debug only, prompt §4.13).

use std::time::Instant;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::middleware::request_id::RequestId;

pub async fn middleware(req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_default();

    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    let latency_ms = started.elapsed().as_millis();

    tracing::info!(
        method = %method,
        path = %path,
        status,
        latency_ms = %latency_ms,
        request_id = %request_id,
        "access"
    );

    resp
}

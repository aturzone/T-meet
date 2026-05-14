//! `GET /ws/:room_id` upgrade.
//!
//! Origin check (prompt §4.13): `WebSocket` upgrades bypass CORS, so reject
//! any upgrade whose `Origin` header doesn't match a host we know about.
//! Without an `Origin` (same-origin curl, integration tests) we let it
//! through — browsers always send it.

use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use crate::app::AppState;
use crate::signaling::conn::run;

pub async fn handler(
    Path(room_id): Path<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_is_acceptable(&headers, &state) {
        let mut resp = Response::new(axum::body::Body::from("bad origin"));
        *resp.status_mut() = StatusCode::FORBIDDEN;
        return resp;
    }
    ws.on_upgrade(move |socket| async move {
        run(socket, room_id, state).await;
    })
}

fn origin_is_acceptable(headers: &HeaderMap, state: &AppState) -> bool {
    let Some(origin) = headers.get(axum::http::header::ORIGIN) else {
        // No Origin → non-browser caller (curl, integration test). Permit.
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    // Accept the configured external host or any IP we bind.
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let bind = state.bind_ip.to_string();
    let external = state.external_host.as_deref();
    host == bind
        || host == "127.0.0.1"
        || host == "localhost"
        || external.is_some_and(|h| h == host)
}

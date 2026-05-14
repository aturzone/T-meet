#![allow(
    clippy::disallowed_methods,
    clippy::manual_let_else,
    clippy::single_match_else
)]

//! `POST /r/:id/join` — public, rate-limited.
//!
//! Returns `401 invalid credentials` for both wrong password and missing room
//! to avoid disclosing room existence. Rate-limit hits return 429.

use std::time::SystemTime;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use meet_core::auth::password;
use meet_core::auth::room_secret;
use meet_core::auth::token::{issue_room, ROOM_TTL_MAX};
use meet_core::db::audit_log::Entry as AuditEntry;
use meet_core::db::rooms;
use meet_core::ids;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::app::AppState;
use crate::middleware::rate_limit::{self, Decision};
use crate::middleware::request_id::RequestId;

#[derive(Debug, Deserialize)]
pub struct JoinReq {
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
pub struct JoinResp {
    pub join_token: String,
    pub ws_url: String,
    pub ice_servers: Vec<serde_json::Value>,
    pub participant_id: String,
}

pub async fn join(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Extension(rid): Extension<RequestId>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<JoinReq>,
) -> Response {
    let peer_ip = connect_info.map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |ci| ci.0.ip());

    // Rate limit before any database work.
    match state.rate_limiter.check(
        peer_ip,
        &room_id,
        rate_limit::JOIN_LIMIT,
        rate_limit::JOIN_WINDOW,
    ) {
        Decision::Allow => {},
        Decision::Deny { retry_after_secs } => {
            return rate_limited(retry_after_secs);
        },
    }

    if req.display_name.is_empty()
        || req.display_name.len() > 64
        || req.display_name.chars().any(char::is_control)
    {
        return audit_and_fail(&state, &rid.0, &room_id, "display_name").await;
    }
    if req.password.is_empty() || req.password.len() > 256 {
        return audit_and_fail(&state, &rid.0, &room_id, "password.bad_length").await;
    }

    let room = match rooms::get(&state.db, &room_id).await {
        Ok(Some(r)) => r,
        _ => {
            return audit_and_fail(&state, &rid.0, &room_id, "unknown_room").await;
        },
    };

    if password::verify(&req.password, &room.password_hash).is_err() {
        return audit_and_fail(&state, &rid.0, &room_id, "wrong_password").await;
    }

    let secret = match room_secret::decrypt(&room.secret_enc, state.at_rest_key.as_ref()) {
        Ok(s) => s,
        Err(_) => {
            tracing::error!(room_id = %room_id, "room_secret decrypt failed");
            return generic_error(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let pid = ids::new_id();
    let token = match issue_room(
        &secret,
        &room_id,
        &pid,
        &req.display_name,
        SystemTime::now(),
        ROOM_TTL_MAX,
    ) {
        Ok(t) => t,
        Err(_) => return generic_error(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let _ = meet_core::db::audit_log::append(
        &state.db,
        &AuditEntry::new(format!("room:{room_id}"), "room.join.success")
            .with_target(&room_id)
            .with_request_id(&rid.0),
    )
    .await;
    tracing::info!(room_id = %room_id, "room joined");

    let body = JoinResp {
        join_token: token,
        ws_url: format!("/ws/{room_id}"),
        ice_servers: Vec::new(),
        participant_id: pid,
    };
    let mut resp = Json(body).into_response();
    *resp.status_mut() = StatusCode::OK;
    resp
}

async fn audit_and_fail(
    state: &AppState,
    rid: &str,
    room_id: &str,
    reason: &'static str,
) -> Response {
    let _ = meet_core::db::audit_log::append(
        &state.db,
        &AuditEntry::new(format!("room:{room_id}"), "room.join.failure")
            .with_target(room_id)
            .with_request_id(rid),
    )
    .await;
    tracing::warn!(room_id = %room_id, reason, "room join failed");
    generic_error(StatusCode::UNAUTHORIZED)
}

fn generic_error(status: StatusCode) -> Response {
    let body = serde_json::json!({"error": "invalid credentials"});
    let mut resp = Json(body).into_response();
    *resp.status_mut() = status;
    resp
}

fn rate_limited(retry_after_secs: u64) -> Response {
    let body = serde_json::json!({"error": "too many attempts"});
    let mut resp = Json(body).into_response();
    *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&retry_after_secs.to_string()) {
        headers.insert(axum::http::header::RETRY_AFTER, v);
    }
    let h = resp.headers_mut();
    for (k, v) in &headers {
        h.insert(k.clone(), v.clone());
    }
    resp
}

// Bridge trait so we can call `.into_response()` on `Json<T>` without a use.
use axum::response::IntoResponse;

//! Compose the axum `Router` with the Phase 02 middleware stack.

use std::sync::Arc;

use axum::http::Method;
use axum::routing::{delete, get, post};
use axum::{middleware as mw, Router};
use meet_core::db::Db;
use meet_core::signaling::sfu_api::SfuPort;

use crate::middleware::{
    access_log, admin_auth, body_limit, rate_limit, request_id, security_headers,
};
use crate::paths::DataPaths;
use crate::routes;
use crate::signaling::room_hub::RoomHub;

/// Shared state. Grows as later phases attach more resources.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub paths: Arc<DataPaths>,
    pub admin_secret: Arc<[u8; 32]>,
    pub at_rest_key: Arc<[u8; 32]>,
    pub rate_limiter: Arc<rate_limit::RateLimiter>,
    pub room_hub: Arc<RoomHub>,
    pub sfu: Arc<dyn SfuPort>,
    pub bind_ip: std::net::IpAddr,
    pub external_host: Option<String>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

pub fn build_app(state: AppState) -> Router {
    let admin_routes = Router::new()
        .route("/rooms", post(routes::admin::create_room))
        .route("/rooms", get(routes::admin::list_rooms))
        .route("/rooms/:id", get(routes::admin::get_room))
        .route("/rooms/:id", delete(routes::admin::delete_room))
        .layer(mw::from_fn_with_state(
            state.clone(),
            admin_auth::middleware,
        ))
        .with_state(state.clone());

    Router::new()
        .route("/healthz", get(routes::health::handler))
        .route(
            "/ca.crt",
            get(routes::ca::handler).with_state(routes::ca::CaSource {
                path: state.paths.ca_public_pem(),
            }),
        )
        .route("/r/:id/join", post(routes::rooms_public::join))
        .route("/api/setup-info", get(routes::setup_info::handler))
        .route("/ws/:room_id", get(routes::ws::handler))
        .nest("/admin", admin_routes)
        .fallback(routes::assets::handler)
        .layer(body_limit::json_layer())
        .layer(mw::from_fn(security_headers::middleware))
        .layer(mw::from_fn(access_log::middleware))
        .layer(mw::from_fn(request_id::middleware))
        .with_state(state)
}

/// Plain-HTTP listener: 301-redirects everything to HTTPS. The one exception
/// is `GET /ca.crt`, which is also served over plain HTTP so locked-down
/// devices that don't yet trust the CA can still pick it up.
pub fn build_redirect_app(
    https_host: String,
    https_port: u16,
    ca_path: std::path::PathBuf,
) -> Router {
    Router::new()
        .route(
            "/ca.crt",
            get(routes::ca::handler).with_state(routes::ca::CaSource { path: ca_path }),
        )
        .fallback(move |req: axum::extract::Request| {
            let host = https_host.clone();
            async move { redirect_handler(&req, &host, https_port) }
        })
        .layer(mw::from_fn(request_id::middleware))
}

fn redirect_handler(
    req: &axum::extract::Request,
    https_host: &str,
    https_port: u16,
) -> axum::response::Response {
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::Response;

    if req.method() != Method::GET && req.method() != Method::HEAD {
        let mut resp = Response::new(axum::body::Body::from("https only"));
        *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        return resp;
    }

    let path_and_query = req
        .uri()
        .path_and_query()
        .map_or_else(|| "/".to_owned(), std::string::ToString::to_string);

    let location = if https_port == 443 {
        format!("https://{https_host}{path_and_query}")
    } else {
        format!("https://{https_host}:{https_port}{path_and_query}")
    };

    let mut resp = Response::new(axum::body::Body::empty());
    *resp.status_mut() = StatusCode::MOVED_PERMANENTLY;
    if let Ok(v) = HeaderValue::from_str(&location) {
        resp.headers_mut().insert(header::LOCATION, v);
    }
    resp
}

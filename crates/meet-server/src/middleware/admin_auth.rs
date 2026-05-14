//! `Authorization: Bearer <paseto>` verification for `/admin/*`.

use std::sync::Arc;
use std::time::SystemTime;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use meet_core::auth::token::verify_admin;

use crate::app::AppState;

#[derive(Debug, Clone)]
pub struct VerifiedAdmin;

pub async fn middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer token"))?;

    let secret: &Arc<[u8; 32]> = &state.admin_secret;
    verify_admin(secret.as_ref(), token, SystemTime::now())
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired token"))?;

    req.extensions_mut().insert(VerifiedAdmin);
    Ok(next.run(req).await)
}

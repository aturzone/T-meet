//! UUID-v4 request id, attached to extensions and echoed in `x-request-id`.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

pub const HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub async fn middleware(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(&HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .map_or_else(|| uuid::Uuid::new_v4().to_string(), str::to_owned);

    req.extensions_mut().insert(RequestId(id.clone()));

    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(&id) {
        resp.headers_mut().insert(HEADER, v);
    }
    resp
}

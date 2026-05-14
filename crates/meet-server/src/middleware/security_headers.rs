//! Baseline security headers (prompt §4.12). Final CSP is locked in Phase 09;
//! everything here is the conservative-starting set so the policy doesn't
//! regress while Phase 02 routes are being added.

use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

const CSP_VALUE: &str = concat!(
    "default-src 'self'; ",
    "connect-src 'self'; ",
    "media-src 'self' blob:; ",
    "img-src 'self' data: blob:; ",
    "style-src 'self' 'unsafe-inline'; ",
    "script-src 'self'; ",
    "frame-ancestors 'none'; ",
    "base-uri 'self'; ",
    "form-action 'self'; ",
    "object-src 'none'",
);

const PERMISSIONS_POLICY: &str =
    "camera=(self), microphone=(self), geolocation=(), interest-cohort=()";

pub async fn middleware(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();

    set(headers, header::CONTENT_SECURITY_POLICY, CSP_VALUE);
    set(
        headers,
        header::STRICT_TRANSPORT_SECURITY,
        "max-age=31536000; includeSubDomains",
    );
    set(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(headers, header::REFERRER_POLICY, "no-referrer");
    set(
        headers,
        HeaderName::from_static("permissions-policy"),
        PERMISSIONS_POLICY,
    );
    set(
        headers,
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    );
    set(
        headers,
        HeaderName::from_static("cross-origin-resource-policy"),
        "same-origin",
    );

    resp
}

fn set(headers: &mut axum::http::HeaderMap, name: HeaderName, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}

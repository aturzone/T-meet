//! Embedded frontend bundle + SPA-friendly fallback.
//!
//! Paths under `/api/`, `/admin/`, `/ws` are reserved — they fall through to a
//! real 404 so client bugs surface. Everything else falls back to `index.html`
//! so React Router can take over on the client.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;

#[derive(rust_embed::Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../frontend/dist/"]
struct Frontend;

const SPA_RESERVED_PREFIXES: &[&str] = &["/api/", "/admin/", "/ws", "/healthz", "/ca.crt"];

const CACHE_HASHED: &str = "public, max-age=31536000, immutable";
const CACHE_INDEX: &str = "no-store";

pub async fn handler(req: Request) -> Response {
    let path = req.uri().path();
    let trimmed = path.trim_start_matches('/');

    if let Some(file) = Frontend::get(trimmed) {
        return file_response(trimmed, file);
    }

    let is_reserved = SPA_RESERVED_PREFIXES
        .iter()
        .any(|p| path.starts_with(p) || path == p.trim_end_matches('/'));

    if is_reserved {
        return (StatusCode::NOT_FOUND, "not found").into_response_owned();
    }

    if let Some(index) = Frontend::get("index.html") {
        return file_response("index.html", index);
    }

    (StatusCode::NOT_FOUND, "frontend not bundled").into_response_owned()
}

fn file_response(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path == "index.html" {
        CACHE_INDEX
    } else {
        CACHE_HASHED
    };

    let mut resp = Response::new(Body::from(file.data));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    resp.headers_mut().insert(
        HeaderName::from_static("etag"),
        HeaderValue::from_str(&format!("\"{}\"", hex_short(&file.metadata.sha256_hash())))
            .unwrap_or_else(|_| HeaderValue::from_static("\"meet\"")),
    );
    resp
}

fn hex_short(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let take = bytes.iter().take(16);
    let mut out = String::with_capacity(32);
    for b in take {
        let _ = write!(out, "{b:02x}");
    }
    out
}

trait IntoResponseOwned {
    fn into_response_owned(self) -> Response;
}

impl<S: Into<String>> IntoResponseOwned for (StatusCode, S) {
    fn into_response_owned(self) -> Response {
        let (status, body) = self;
        let mut resp = Response::new(Body::from(body.into()));
        *resp.status_mut() = status;
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        resp
    }
}

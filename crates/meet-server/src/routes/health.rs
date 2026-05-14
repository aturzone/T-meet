use axum::http::StatusCode;

pub async fn handler() -> (StatusCode, &'static str) {
    (StatusCode::OK, "ok")
}

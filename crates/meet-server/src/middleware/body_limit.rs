//! Per-route body size caps. Phase 02 sets a 1 MiB JSON limit and a smaller
//! 64 KiB limit for `/r/:id/join` (handler lands in Phase 03, layer here).

use tower_http::limit::RequestBodyLimitLayer;

pub const JSON_LIMIT: usize = 1 << 20; // 1 MiB
pub const JOIN_LIMIT: usize = 64 << 10; // 64 KiB

#[must_use]
pub fn json_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(JSON_LIMIT)
}

#[must_use]
pub fn join_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(JOIN_LIMIT)
}

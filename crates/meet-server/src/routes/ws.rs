//! `GET /ws/:room_id` upgrade.

use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::Response;

use crate::app::AppState;
use crate::signaling::conn::run;

pub async fn handler(
    Path(room_id): Path<String>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| async move {
        run(socket, room_id, state).await;
    })
}

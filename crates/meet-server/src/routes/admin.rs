//! `/admin/*` endpoints. All gated by [`crate::middleware::admin_auth`].
//!
//! Endpoints:
//! - `POST /admin/rooms`         create
//! - `GET  /admin/rooms`         list
//! - `GET  /admin/rooms/:id`     fetch one
//! - `DELETE /admin/rooms/:id`   delete

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use meet_core::auth::password;
use meet_core::auth::room_secret;
use meet_core::db::audit_log::Entry as AuditEntry;
use meet_core::db::rooms::{self, Room};
use meet_core::ids;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::app::AppState;
use crate::middleware::admin_auth::VerifiedAdmin;
use crate::middleware::request_id::RequestId;

#[derive(Debug, Deserialize)]
pub struct CreateRoomReq {
    pub name: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub creator_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateRoomResp {
    pub id: String,
    pub name: String,
    /// Plaintext password. Returned ONCE in this response only.
    pub password: String,
    pub join_url: String,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub creator_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListRoomsResp {
    pub rooms: Vec<RoomSummary>,
}

pub async fn create_room(
    State(state): State<AppState>,
    Extension(_admin): Extension<VerifiedAdmin>,
    Extension(rid): Extension<RequestId>,
    Json(req): Json<CreateRoomReq>,
) -> Result<(StatusCode, Json<CreateRoomResp>), (StatusCode, &'static str)> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err((StatusCode::BAD_REQUEST, "name: 1..=100 chars"));
    }
    if let Some(p) = &req.password {
        if p.len() < 4 || p.len() > 256 {
            return Err((StatusCode::BAD_REQUEST, "password: 4..=256 chars"));
        }
    }
    if let Some(note) = &req.creator_note {
        if note.len() > 500 {
            return Err((StatusCode::BAD_REQUEST, "creator_note: ≤500 chars"));
        }
    }

    let password = req.password.unwrap_or_else(ids::new_room_password);
    let hash = password::hash(&password).map_err(|_| internal())?;

    // Per-room secret, sealed at rest with the admin-passphrase key.
    let secret = room_secret::generate();
    let secret_enc =
        room_secret::encrypt(&secret, state.at_rest_key.as_ref()).map_err(|_| internal())?;

    let room = Room {
        id: ids::new_id(),
        name: req.name.clone(),
        password_hash: hash,
        // PHC string above already embeds the salt; the column is kept for the
        // chacha20poly1305 nonce salt — we reuse the room id as derivation
        // salt seed for future use. v1 stores 16 random bytes.
        salt: random_salt(),
        secret_enc,
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
        expires_at: req.expires_at,
        creator_note: req.creator_note.clone(),
    };

    rooms::insert(&state.db, &room)
        .await
        .map_err(|_| internal())?;

    let _ = meet_core::db::audit_log::append(
        &state.db,
        &AuditEntry::new("admin", "room.create")
            .with_target(&room.id)
            .with_request_id(&rid.0),
    )
    .await;

    let join_url = format!("/r/{}", room.id);
    let resp = CreateRoomResp {
        id: room.id.clone(),
        name: room.name,
        password,
        join_url,
        expires_at: room.expires_at,
    };
    tracing::info!(room_id = %room.id, "room created");
    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn list_rooms(
    State(state): State<AppState>,
    Extension(_admin): Extension<VerifiedAdmin>,
) -> Result<Json<ListRoomsResp>, (StatusCode, &'static str)> {
    let rooms = rooms::list(&state.db).await.map_err(|_| internal())?;
    let summary = rooms
        .into_iter()
        .map(|r| RoomSummary {
            id: r.id,
            name: r.name,
            created_at: r.created_at,
            expires_at: r.expires_at,
            creator_note: r.creator_note,
        })
        .collect();
    Ok(Json(ListRoomsResp { rooms: summary }))
}

pub async fn get_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(_admin): Extension<VerifiedAdmin>,
) -> Result<Json<RoomSummary>, (StatusCode, &'static str)> {
    let r = rooms::get(&state.db, &id)
        .await
        .map_err(|_| internal())?
        .ok_or((StatusCode::NOT_FOUND, "no such room"))?;
    Ok(Json(RoomSummary {
        id: r.id,
        name: r.name,
        created_at: r.created_at,
        expires_at: r.expires_at,
        creator_note: r.creator_note,
    }))
}

pub async fn delete_room(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(_admin): Extension<VerifiedAdmin>,
    Extension(rid): Extension<RequestId>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let removed = rooms::delete(&state.db, &id)
        .await
        .map_err(|_| internal())?;
    if !removed {
        return Err((StatusCode::NOT_FOUND, "no such room"));
    }
    let _ = meet_core::db::audit_log::append(
        &state.db,
        &AuditEntry::new("admin", "room.delete")
            .with_target(&id)
            .with_request_id(&rid.0),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

fn internal() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

fn random_salt() -> Vec<u8> {
    use rand::{rngs::OsRng, RngCore};
    let mut s = vec![0u8; 16];
    OsRng.fill_bytes(&mut s);
    s
}

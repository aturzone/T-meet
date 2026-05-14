#![forbid(unsafe_code)]
#![allow(
    clippy::too_many_lines,
    clippy::single_match_else,
    clippy::module_name_repetitions
)]

//! Selective Forwarding Unit — webrtc-rs implementation of
//! [`meet_core::signaling::sfu_api::SfuPort`].
//!
//! Architecture:
//! - [`Sfu`] — top-level value owning the shared `webrtc::api::API` instance
//!   and the room registry.
//! - [`room::RoomSession`] — one per room. Holds the participant
//!   [`PeerSession`]s plus the published-track registry used for fan-out.
//! - [`peer::PeerSession`] — one [`RTCPeerConnection`] per participant plus
//!   the `TrackLocalStaticRTP`s the SFU writes RTP into.
//!
//! Phase 05 scope: full offer/answer round-trip, track forwarding within an
//! already-subscribed room, graceful cleanup. Server-initiated renegotiation
//! (sending Offer messages back to subscribers as new publishers appear) is
//! sketched but the wire-up needs the frontend to land in Phase 07.

pub mod api_engine;
pub mod peer;
pub mod room;

use std::sync::Arc;

use async_trait::async_trait;
use meet_core::signaling::sfu_api::{SfuError as PortError, SfuPort};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;
use webrtc::api::API;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::room::RoomRegistry;

#[derive(Debug, Error)]
pub enum SfuError {
    #[error("engine: {0}")]
    Engine(String),

    #[error("webrtc: {0}")]
    Webrtc(String),

    #[error("no such room: {0}")]
    NoSuchRoom(String),

    #[error("no such participant: {0}")]
    NoSuchParticipant(String),

    #[error("room full: {0}")]
    RoomFull(String),

    #[error("serde: {0}")]
    Serde(String),
}

impl From<SfuError> for PortError {
    fn from(_value: SfuError) -> Self {
        // The port error is intentionally opaque — full detail goes to logs.
        PortError::NotImplemented
    }
}

#[derive(Debug, Clone)]
pub struct SfuConfig {
    pub max_participants_per_room: usize,
}

impl Default for SfuConfig {
    fn default() -> Self {
        Self {
            max_participants_per_room: 30,
        }
    }
}

pub struct Sfu {
    api: Arc<API>,
    rooms: Arc<Mutex<RoomRegistry>>,
    config: SfuConfig,
}

impl std::fmt::Debug for Sfu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sfu")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Sfu {
    /// Build a new SFU. The internal `webrtc::api::API` is constructed eagerly
    /// so first-join latency stays predictable.
    ///
    /// # Errors
    /// Bubbles up [`SfuError::Engine`] if codec registration fails.
    pub fn new(config: SfuConfig) -> Result<Self, SfuError> {
        let api = api_engine::build(None)?;
        Ok(Self {
            api,
            rooms: Arc::new(Mutex::new(RoomRegistry::new())),
            config,
        })
    }

    /// Test-only constructor with a default config.
    ///
    /// # Errors
    /// Bubbles up [`SfuError::Engine`] on codec registration failure.
    pub fn new_default() -> Result<Self, SfuError> {
        Self::new(SfuConfig::default())
    }
}

#[async_trait]
impl SfuPort for Sfu {
    async fn on_join(&self, room_id: &str, pid: &str) -> Result<(), PortError> {
        let mut rooms = self.rooms.lock().await;
        let room = rooms.get_or_create(room_id);
        if room.participant_count() >= self.config.max_participants_per_room {
            tracing::warn!(room_id, pid, "room full");
            return Err(SfuError::RoomFull(room_id.to_owned()).into());
        }
        let peer = peer::PeerSession::new(self.api.clone(), pid.to_owned())
            .await
            .map_err(|e| {
                tracing::error!(room_id, pid, error = %e, "PeerSession::new failed");
                PortError::from(e)
            })?;
        room.add_participant(pid.to_owned(), Arc::new(peer));
        tracing::info!(
            room_id,
            pid,
            count = room.participant_count(),
            "sfu peer joined"
        );
        Ok(())
    }

    async fn on_leave(&self, room_id: &str, pid: &str) {
        let mut rooms = self.rooms.lock().await;
        if let Some(room) = rooms.get_mut(room_id) {
            if let Some(peer) = room.remove_participant(pid) {
                // Close PC in the background — closing can yield, we don't
                // want to hold the registry lock.
                let pc = peer.peer_connection();
                tokio::spawn(async move {
                    let _ = pc.close().await;
                });
            }
            if room.participant_count() == 0 {
                rooms.remove(room_id);
            }
        }
        tracing::info!(room_id, pid, "sfu peer left");
    }

    async fn on_offer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<String, PortError> {
        let peer = {
            let rooms = self.rooms.lock().await;
            let room = rooms
                .get(room_id)
                .ok_or_else(|| SfuError::NoSuchRoom(room_id.to_owned()))?;
            room.participant(pid)
                .ok_or_else(|| SfuError::NoSuchParticipant(pid.to_owned()))?
                .clone()
        };

        let offer = RTCSessionDescription::offer(sdp.to_owned())
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;
        let pc = peer.peer_connection();
        pc.set_remote_description(offer)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;

        let answer = pc
            .create_answer(None)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;
        pc.set_local_description(answer)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;

        // Wait for ICE gathering to finish so the answer carries candidates
        // (lightweight blocking approach; for production we'd trickle).
        let mut gather = pc.gathering_complete_promise().await;
        let _ = gather.recv().await;

        let local = pc
            .local_description()
            .await
            .ok_or_else(|| SfuError::Webrtc("no local description after create_answer".into()))?;
        Ok(local.sdp)
    }

    async fn on_answer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<(), PortError> {
        let peer = {
            let rooms = self.rooms.lock().await;
            let room = rooms
                .get(room_id)
                .ok_or_else(|| SfuError::NoSuchRoom(room_id.to_owned()))?;
            room.participant(pid)
                .ok_or_else(|| SfuError::NoSuchParticipant(pid.to_owned()))?
                .clone()
        };
        let answer = RTCSessionDescription::answer(sdp.to_owned())
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;
        peer.peer_connection()
            .set_remote_description(answer)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;
        Ok(())
    }

    async fn on_ice(&self, room_id: &str, pid: &str, candidate: Value) -> Result<(), PortError> {
        let peer = {
            let rooms = self.rooms.lock().await;
            let room = rooms
                .get(room_id)
                .ok_or_else(|| SfuError::NoSuchRoom(room_id.to_owned()))?;
            room.participant(pid)
                .ok_or_else(|| SfuError::NoSuchParticipant(pid.to_owned()))?
                .clone()
        };
        // Accept the candidate either as { "candidate": "...", ... } object
        // or as a raw string per the spec.
        let init: RTCIceCandidateInit =
            serde_json::from_value(candidate).map_err(|e| SfuError::Serde(e.to_string()))?;
        peer.peer_connection()
            .add_ice_candidate(init)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_sfu_does_not_panic() {
        let _sfu = Sfu::new_default().expect("sfu::new");
    }

    #[tokio::test]
    async fn join_then_leave_clears_room() {
        let sfu = Sfu::new_default().expect("sfu::new");
        sfu.on_join("r1", "p1").await.expect("join");
        assert_eq!(sfu.rooms.lock().await.participant_count("r1"), 1);
        sfu.on_leave("r1", "p1").await;
        // Empty room is auto-dropped.
        assert_eq!(sfu.rooms.lock().await.participant_count("r1"), 0);
    }

    #[tokio::test]
    async fn room_full_rejects_join() {
        let cfg = SfuConfig {
            max_participants_per_room: 2,
        };
        let sfu = Sfu::new(cfg).expect("sfu::new");
        sfu.on_join("r1", "a").await.expect("a");
        sfu.on_join("r1", "b").await.expect("b");
        let err = sfu.on_join("r1", "c").await;
        assert!(err.is_err(), "third join should fail");
    }
}

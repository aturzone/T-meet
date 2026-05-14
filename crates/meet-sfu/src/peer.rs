//! Per-participant peer connection.
//!
//! Wraps an [`RTCPeerConnection`] and tracks the connection state. Published
//! tracks from this peer are forwarded to other peers in the same room by the
//! `on_track` callback (wired up in `Sfu::on_join` once the room registry has
//! the cross-peer view).
//!
//! Phase 05 keeps the forwarding logic *inside* `on_track`'s capture set
//! intentionally minimal: each subscriber must have set up a recvonly
//! transceiver in their initial offer. Full server-initiated renegotiation
//! lands once Phase 07 (frontend WebRTC) supplies the signaling roundtrip.

use std::sync::Arc;

use webrtc::api::API;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

use crate::SfuError;

#[derive(Debug)]
pub struct PeerSession {
    pid: String,
    pc: Arc<RTCPeerConnection>,
}

impl PeerSession {
    /// Build a new `PeerSession` against the shared `API`.
    ///
    /// # Errors
    /// Bubbles up [`SfuError::Webrtc`] on peer-connection construction failure.
    pub async fn new(api: Arc<API>, pid: String) -> Result<Self, SfuError> {
        let config = RTCConfiguration {
            // No STUN/TURN in v1 — host candidates only, suitable for LAN and
            // 1:1-NAT deployments. Operators add external_ip for the latter.
            ice_servers: Vec::new(),
            ..Default::default()
        };
        let pc = api
            .new_peer_connection(config)
            .await
            .map_err(|e| SfuError::Webrtc(e.to_string()))?;

        // Lightweight state logging — never includes PII.
        let pid_for_log = pid.clone();
        pc.on_peer_connection_state_change(Box::new(move |state| {
            let pid = pid_for_log.clone();
            Box::pin(async move {
                tracing::debug!(pid = %pid, state = ?state, "pc state");
            })
        }));

        Ok(Self {
            pid,
            pc: Arc::new(pc),
        })
    }

    #[must_use]
    pub fn pid(&self) -> &str {
        &self.pid
    }

    #[must_use]
    pub fn peer_connection(&self) -> Arc<RTCPeerConnection> {
        self.pc.clone()
    }
}

//! Wire types + state machine for the WebSocket signaling channel.
//!
//! The protocol is documented in `docs/plan/phase-04-signaling.md`. All
//! messages are versioned JSON with a `type` discriminator and a `v: 1`
//! envelope (kept as an explicit field so the schema can evolve without a
//! whole-channel break).

pub mod sfu_api;
pub mod state;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

/// Hard maximum size of a single signaling frame (prompt §4.13). Frames
/// larger than this close the connection with [`CloseCode::MessageTooLarge`].
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Heartbeat tick / idle timeout (prompt phase doc).
pub const HEARTBEAT_INTERVAL_SECS: u64 = 20;
pub const IDLE_TIMEOUT_SECS: u64 = 60;

/// Outbound channel cap per connection. Anything above this closes the socket
/// with [`CloseCode::Backpressure`].
pub const OUTBOUND_QUEUE_CAP: usize = 64;

/// Close codes used by the signaling layer.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseCode {
    Normal = 1000,
    ProtocolViolation = 4400,
    AuthFailure = 4401,
    IdleTimeout = 4408,
    Replaced = 4409,
    MessageTooLarge = 4413,
    Backpressure = 4429,
    RoomFull = 4453,
}

impl CloseCode {
    #[must_use]
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Peer descriptor used by `Joined` / `PeerJoined`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerDescriptor {
    pub pid: String,
    pub display_name: String,
    /// Filled in once Phase 08 (chat) lands. `None` for now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDescriptor {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// First message after the WS upgrade. Must arrive within 5 s.
    Join {
        #[serde(default = "default_v")]
        v: u32,
        token: String,
    },
    Offer {
        #[serde(default = "default_v")]
        v: u32,
        sdp: String,
        to: String,
    },
    Answer {
        #[serde(default = "default_v")]
        v: u32,
        sdp: String,
        to: String,
    },
    IceCandidate {
        #[serde(default = "default_v")]
        v: u32,
        candidate: Value,
        to: String,
    },
    Chat {
        #[serde(default = "default_v")]
        v: u32,
        ciphertext: String,
        nonce: String,
        to: String,
    },
    Ping {
        #[serde(default = "default_v")]
        v: u32,
        ts: i64,
    },
    /// Tell the server "this is my public key — please tell every peer."
    /// Phase 08: X25519 public key for sealed-box chat.
    Announce {
        #[serde(default = "default_v")]
        v: u32,
        pubkey: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    Joined {
        v: u32,
        you: PeerDescriptor,
        peers: Vec<PeerDescriptor>,
        room: RoomDescriptor,
    },
    PeerJoined {
        v: u32,
        peer: PeerDescriptor,
    },
    /// A peer's descriptor changed (e.g. they announced their pubkey).
    PeerUpdated {
        v: u32,
        peer: PeerDescriptor,
    },
    PeerLeft {
        v: u32,
        pid: String,
    },
    Offer {
        v: u32,
        sdp: String,
        from: String,
    },
    Answer {
        v: u32,
        sdp: String,
        from: String,
    },
    IceCandidate {
        v: u32,
        candidate: Value,
        from: String,
    },
    Chat {
        v: u32,
        ciphertext: String,
        nonce: String,
        from: String,
    },
    Pong {
        v: u32,
        ts_client: i64,
        ts_server: i64,
    },
    Error {
        v: u32,
        code: u32,
        message: String,
    },
}

fn default_v() -> u32 {
    PROTOCOL_VERSION
}

impl ServerMsg {
    #[must_use]
    pub fn joined(you: PeerDescriptor, peers: Vec<PeerDescriptor>, room: RoomDescriptor) -> Self {
        Self::Joined {
            v: PROTOCOL_VERSION,
            you,
            peers,
            room,
        }
    }

    #[must_use]
    pub fn peer_joined(peer: PeerDescriptor) -> Self {
        Self::PeerJoined {
            v: PROTOCOL_VERSION,
            peer,
        }
    }

    #[must_use]
    pub fn peer_updated(peer: PeerDescriptor) -> Self {
        Self::PeerUpdated {
            v: PROTOCOL_VERSION,
            peer,
        }
    }

    #[must_use]
    pub fn peer_left(pid: impl Into<String>) -> Self {
        Self::PeerLeft {
            v: PROTOCOL_VERSION,
            pid: pid.into(),
        }
    }

    #[must_use]
    pub fn pong(ts_client: i64, ts_server: i64) -> Self {
        Self::Pong {
            v: PROTOCOL_VERSION,
            ts_client,
            ts_server,
        }
    }

    #[must_use]
    pub fn error(code: CloseCode, message: impl Into<String>) -> Self {
        Self::Error {
            v: PROTOCOL_VERSION,
            code: u32::from(code.as_u16()),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn chat(ciphertext: String, nonce: String, from: String) -> Self {
        Self::Chat {
            v: PROTOCOL_VERSION,
            ciphertext,
            nonce,
            from,
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn client_join_round_trip() {
        let raw = r#"{"type":"Join","v":1,"token":"v4.local.abc"}"#;
        let msg: ClientMsg = serde_json::from_str(raw).expect("parse");
        match msg {
            ClientMsg::Join { v, token } => {
                assert_eq!(v, 1);
                assert_eq!(token, "v4.local.abc");
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn unknown_variant_rejected() {
        let raw = r#"{"type":"Nope","v":1}"#;
        assert!(serde_json::from_str::<ClientMsg>(raw).is_err());
    }

    #[test]
    fn server_joined_serializes_with_pubkey_omitted_when_none() {
        let msg = ServerMsg::joined(
            PeerDescriptor {
                pid: "p1".into(),
                display_name: "Alice".into(),
                pubkey: None,
            },
            vec![],
            RoomDescriptor {
                id: "r1".into(),
                name: "test".into(),
            },
        );
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"Joined\""));
        assert!(!json.contains("pubkey"), "pubkey None should be omitted");
    }

    #[test]
    fn server_error_carries_code_and_message() {
        let msg = ServerMsg::error(CloseCode::AuthFailure, "bad token");
        let v = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(v["code"], 4401);
        assert_eq!(v["message"], "bad token");
    }

    #[test]
    fn chat_round_trip() {
        let raw = r#"{"type":"Chat","v":1,"ciphertext":"AA==","nonce":"BB==","to":"all"}"#;
        let msg: ClientMsg = serde_json::from_str(raw).expect("parse");
        match msg {
            ClientMsg::Chat { ciphertext, to, .. } => {
                assert_eq!(ciphertext, "AA==");
                assert_eq!(to, "all");
            },
            _ => panic!("wrong variant"),
        }
    }
}

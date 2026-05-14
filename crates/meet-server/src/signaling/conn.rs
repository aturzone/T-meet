//! Per-connection task. Owns the WS socket, the outbound mpsc, and the
//! state machine. One Tokio task per peer.
//!
//! Auth contract:
//! - Upgrade arrives at `GET /ws/:room_id`. The room id is public.
//! - The PASETO join token arrives in the FIRST WS message (`Join`) and is
//!   verified against `room_id`'s per-room secret. Tokens never appear in
//!   URLs or logs.

#![allow(
    clippy::too_many_lines,
    clippy::manual_let_else,
    clippy::single_match_else,
    clippy::question_mark,
    clippy::map_unwrap_or,
    clippy::useless_conversion,
    clippy::implicit_clone
)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use futures::stream::StreamExt;
use futures::SinkExt;
use meet_core::db::rooms;
use meet_core::signaling::state::{transition, Action, ConnState, Event};
use meet_core::signaling::{
    ClientMsg, CloseCode, PeerDescriptor, RoomDescriptor, ServerMsg, HEARTBEAT_INTERVAL_SECS,
    IDLE_TIMEOUT_SECS, MAX_FRAME_BYTES,
};
use tokio::sync::mpsc;
use tokio::time::interval;

use crate::app::AppState;
use crate::signaling::room_hub::{make_outbound, Participant};

const FIRST_MESSAGE_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn run(socket: WebSocket, room_id: String, state: AppState) {
    let mut handler = Handler::new(state, room_id);
    if let Err(reason) = handler.run(socket).await {
        tracing::debug!(reason = %reason, "ws connection ended");
    }
}

struct Handler {
    state: AppState,
    room_id: String,
    session_pid: Option<String>,
    conn_state: ConnState,
}

impl Handler {
    fn new(state: AppState, room_id: String) -> Self {
        Self {
            state,
            room_id,
            session_pid: None,
            conn_state: ConnState::Connecting,
        }
    }

    async fn run(&mut self, socket: WebSocket) -> Result<(), &'static str> {
        let (mut ws_tx, mut ws_rx) = socket.split();
        let (out_tx, out_rx) = make_outbound();

        // First-message auth.
        let first = tokio::time::timeout(FIRST_MESSAGE_TIMEOUT, ws_rx.next()).await;
        let join_msg = match first {
            Ok(Some(Ok(Message::Text(t)))) => t.to_string(),
            Ok(Some(Ok(Message::Binary(_)))) => {
                let _ = ws_tx
                    .send(close_message(CloseCode::ProtocolViolation, "expected text"))
                    .await;
                return Err("binary first frame");
            },
            _ => {
                let _ = ws_tx
                    .send(close_message(CloseCode::AuthFailure, "no Join within 5s"))
                    .await;
                return Err("auth timeout");
            },
        };
        if join_msg.len() > MAX_FRAME_BYTES {
            let _ = ws_tx
                .send(close_message(CloseCode::MessageTooLarge, "frame too large"))
                .await;
            return Err("frame too large");
        }

        let token = match serde_json::from_str::<ClientMsg>(&join_msg) {
            Ok(ClientMsg::Join { token, .. }) => token,
            Ok(_) => {
                let _ = ws_tx
                    .send(close_message(
                        CloseCode::ProtocolViolation,
                        "first message must be Join",
                    ))
                    .await;
                return Err("first message was not Join");
            },
            Err(_) => {
                let _ = ws_tx
                    .send(close_message(CloseCode::ProtocolViolation, "bad json"))
                    .await;
                return Err("bad json");
            },
        };

        // Resolve room + decrypt its secret.
        let room = match rooms::get(&self.state.db, &self.room_id).await {
            Ok(Some(r)) => r,
            _ => {
                let _ = ws_tx
                    .send(close_message(CloseCode::AuthFailure, "invalid"))
                    .await;
                return Err("unknown room");
            },
        };
        let secret = match meet_core::auth::room_secret::decrypt(
            &room.secret_enc,
            self.state.at_rest_key.as_ref(),
        ) {
            Ok(s) => s,
            Err(_) => {
                let _ = ws_tx
                    .send(close_message(CloseCode::AuthFailure, "invalid"))
                    .await;
                return Err("room secret decrypt failed");
            },
        };
        let claims = match meet_core::auth::token::verify_room(
            &secret,
            &self.room_id,
            &token,
            SystemTime::now(),
        ) {
            Ok(c) => c,
            Err(_) => {
                let _ = ws_tx
                    .send(close_message(CloseCode::AuthFailure, "invalid"))
                    .await;
                return Err("token invalid");
            },
        };

        // State machine: Join → AuthOk → InRoom.
        for ev in [Event::JoinReceived, Event::AuthOk] {
            self.conn_state = match transition(self.conn_state, ev) {
                Action::Continue(s) => s,
                Action::Close(_, _) => return Err("state refused"),
            };
        }

        let descriptor = PeerDescriptor {
            pid: claims.pid.clone(),
            display_name: claims.display_name.clone(),
            pubkey: None,
        };

        let snap = self.state.room_hub.join(
            &self.room_id,
            Participant {
                descriptor: descriptor.clone(),
                outbound: out_tx.clone(),
            },
        );
        if let Some(stale) = snap.replaced_outbound {
            let _ = stale.try_send(ServerMsg::error(
                CloseCode::Replaced,
                "replaced by new connection",
            ));
        }

        self.session_pid = Some(claims.pid.clone());

        // Tell this socket who's here.
        let joined = ServerMsg::joined(
            descriptor.clone(),
            snap.peers,
            RoomDescriptor {
                id: self.room_id.clone(),
                name: room.name.clone(),
            },
        );
        if let Err(e) = send_json(&mut ws_tx, &joined).await {
            return Err(e);
        }

        // Tell everyone else.
        self.state.room_hub.broadcast(
            &self.room_id,
            &claims.pid,
            ServerMsg::peer_joined(descriptor),
        );

        let _ = self.state.sfu.on_join(&self.room_id, &claims.pid).await;

        let result = self.run_loop(ws_tx, ws_rx, out_rx).await;

        // Cleanup.
        if let Some(pid) = self.session_pid.take() {
            self.state.room_hub.leave(&self.room_id, &pid);
            self.state
                .room_hub
                .broadcast(&self.room_id, &pid, ServerMsg::peer_left(pid.clone()));
            self.state.sfu.on_leave(&self.room_id, &pid).await;
        }
        result
    }

    async fn run_loop(
        &mut self,
        mut ws_tx: futures::stream::SplitSink<WebSocket, Message>,
        mut ws_rx: futures::stream::SplitStream<WebSocket>,
        mut out_rx: mpsc::Receiver<ServerMsg>,
    ) -> Result<(), &'static str> {
        let mut heartbeat = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        let mut last_seen = std::time::Instant::now();

        loop {
            tokio::select! {
                Some(msg) = out_rx.recv() => {
                    if let Err(e) = send_json(&mut ws_tx, &msg).await {
                        return Err(e);
                    }
                }
                inbound = ws_rx.next() => {
                    last_seen = std::time::Instant::now();
                    match inbound {
                        Some(Ok(Message::Text(t))) => {
                            let raw = t.to_string();
                            if raw.len() > MAX_FRAME_BYTES {
                                let _ = ws_tx
                                    .send(close_message(CloseCode::MessageTooLarge, "frame too large"))
                                    .await;
                                return Err("frame too large");
                            }
                            if let Err(e) = self.handle_client_msg(&raw, &mut ws_tx).await {
                                return Err(e);
                            }
                        }
                        Some(Ok(Message::Ping(p))) => {
                            let _ = ws_tx.send(Message::Pong(p)).await;
                        }
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(Message::Close(_))) | None => return Ok(()),
                        Some(Ok(Message::Binary(_))) => {
                            let _ = ws_tx
                                .send(close_message(CloseCode::ProtocolViolation, "binary not allowed"))
                                .await;
                            return Err("binary frame");
                        }
                        Some(Err(_)) => return Err("ws error"),
                    }
                }
                _ = heartbeat.tick() => {
                    if last_seen.elapsed() >= Duration::from_secs(IDLE_TIMEOUT_SECS) {
                        let _ = ws_tx
                            .send(close_message(CloseCode::IdleTimeout, "idle"))
                            .await;
                        return Err("idle");
                    }
                }
            }
        }
    }

    async fn handle_client_msg(
        &mut self,
        raw: &str,
        ws_tx: &mut futures::stream::SplitSink<WebSocket, Message>,
    ) -> Result<(), &'static str> {
        let msg = match serde_json::from_str::<ClientMsg>(raw) {
            Ok(m) => m,
            Err(_) => {
                let _ = ws_tx
                    .send(close_message(CloseCode::ProtocolViolation, "bad json"))
                    .await;
                return Err("bad json");
            },
        };

        let pid = self.session_pid.clone().ok_or("no session")?;

        match msg {
            ClientMsg::Join { .. } => {
                let _ = ws_tx
                    .send(close_message(CloseCode::ProtocolViolation, "double join"))
                    .await;
                return Err("double join");
            },
            ClientMsg::Ping { ts, .. } => {
                let server_ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
                    .unwrap_or(0);
                self.state
                    .room_hub
                    .send_to(&self.room_id, &pid, ServerMsg::pong(ts, server_ts));
            },
            ClientMsg::Chat {
                ciphertext,
                nonce,
                to,
                ..
            } => {
                if ciphertext.len() > MAX_FRAME_BYTES / 2 {
                    let _ = ws_tx
                        .send(close_message(CloseCode::MessageTooLarge, "chat too large"))
                        .await;
                    return Err("chat too large");
                }
                let out = ServerMsg::chat(ciphertext, nonce, pid.clone());
                if to == "all" {
                    self.state.room_hub.broadcast(&self.room_id, &pid, out);
                } else {
                    self.state.room_hub.send_to(&self.room_id, &to, out);
                }
            },
            ClientMsg::Offer { sdp, .. } => {
                match self.state.sfu.on_offer(&self.room_id, &pid, &sdp).await {
                    Ok(answer_sdp) => {
                        let resp = ServerMsg::Answer {
                            v: meet_core::signaling::PROTOCOL_VERSION,
                            sdp: answer_sdp,
                            from: "sfu".into(),
                        };
                        self.state.room_hub.send_to(&self.room_id, &pid, resp);
                    },
                    Err(e) => {
                        tracing::warn!(room_id = %self.room_id, pid = %pid, error = ?e, "sfu on_offer failed");
                        self.state.room_hub.send_to(
                            &self.room_id,
                            &pid,
                            ServerMsg::error(CloseCode::ProtocolViolation, "offer rejected"),
                        );
                    },
                }
            },
            ClientMsg::Answer { sdp, .. } => {
                if let Err(e) = self.state.sfu.on_answer(&self.room_id, &pid, &sdp).await {
                    tracing::warn!(room_id = %self.room_id, pid = %pid, error = ?e, "sfu on_answer failed");
                }
            },
            ClientMsg::IceCandidate { candidate, .. } => {
                let _ = self.state.sfu.on_ice(&self.room_id, &pid, candidate).await;
            },
        }
        Ok(())
    }
}

async fn send_json(
    ws_tx: &mut futures::stream::SplitSink<WebSocket, Message>,
    msg: &ServerMsg,
) -> Result<(), &'static str> {
    let json = serde_json::to_string(msg).map_err(|_| "serialize failed")?;
    ws_tx
        .send(Message::Text(json.into()))
        .await
        .map_err(|_| "ws send failed")
}

fn close_message(code: CloseCode, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code: code.as_u16(),
        reason: reason.into(),
    }))
}

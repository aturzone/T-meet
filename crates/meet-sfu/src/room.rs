//! Per-room state: participant registry + published-track map.
//!
//! `HashMap` is fine here — the keys are server-issued opaque ids, not
//! attacker-controlled input.

#![allow(clippy::disallowed_types)]

use std::collections::HashMap;
use std::sync::Arc;

use crate::peer::PeerSession;

#[derive(Debug, Default)]
pub struct RoomSession {
    participants: HashMap<String, Arc<PeerSession>>,
}

impl RoomSession {
    pub fn add_participant(&mut self, pid: String, peer: Arc<PeerSession>) {
        self.participants.insert(pid, peer);
    }

    pub fn remove_participant(&mut self, pid: &str) -> Option<Arc<PeerSession>> {
        self.participants.remove(pid)
    }

    #[must_use]
    pub fn participant(&self, pid: &str) -> Option<&Arc<PeerSession>> {
        self.participants.get(pid)
    }

    #[must_use]
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

#[derive(Debug, Default)]
pub struct RoomRegistry {
    rooms: HashMap<String, RoomSession>,
}

impl RoomRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_create(&mut self, room_id: &str) -> &mut RoomSession {
        self.rooms.entry(room_id.to_owned()).or_default()
    }

    #[must_use]
    pub fn get(&self, room_id: &str) -> Option<&RoomSession> {
        self.rooms.get(room_id)
    }

    pub fn get_mut(&mut self, room_id: &str) -> Option<&mut RoomSession> {
        self.rooms.get_mut(room_id)
    }

    pub fn remove(&mut self, room_id: &str) -> Option<RoomSession> {
        self.rooms.remove(room_id)
    }

    #[must_use]
    pub fn participant_count(&self, room_id: &str) -> usize {
        self.rooms
            .get(room_id)
            .map_or(0, RoomSession::participant_count)
    }
}

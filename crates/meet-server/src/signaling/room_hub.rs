#![allow(clippy::disallowed_types)]

//! Per-room broadcast registry. Keyed by room id; each room owns the set of
//! live participants and the channels we fan messages out on.
//!
//! Bounded mpsc channels for outbound delivery — slow consumers get dropped
//! (see [`crate::signaling::conn`]) rather than holding the room hostage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use meet_core::signaling::{PeerDescriptor, ServerMsg, OUTBOUND_QUEUE_CAP};
use tokio::sync::mpsc;

/// Outbound queue for one participant. Falls behind → connection closes.
pub type Outbound = mpsc::Sender<ServerMsg>;

#[derive(Debug)]
pub struct Participant {
    pub descriptor: PeerDescriptor,
    pub outbound: Outbound,
}

#[derive(Debug, Default)]
pub struct Room {
    pub participants: HashMap<String, Participant>,
}

#[derive(Debug, Default)]
pub struct RoomHub {
    rooms: Mutex<HashMap<String, Room>>,
}

/// Returned from [`RoomHub::join`] so the connection knows who else is here
/// AND the previously-registered Sender (so we can close the duplicate pid
/// with [`CloseCode::Replaced`]).
pub struct JoinSnapshot {
    pub peers: Vec<PeerDescriptor>,
    /// `Some` only when a previous connection used the same pid.
    pub replaced_outbound: Option<Outbound>,
}

impl RoomHub {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `participant`. Returns the current peer list (excluding self),
    /// and — if there was a stale connection for the same pid — its outbound
    /// channel so the caller can send a final Error and close it.
    pub fn join(&self, room_id: &str, participant: Participant) -> JoinSnapshot {
        let mut guard = self.lock();
        let room = guard.entry(room_id.to_owned()).or_default();
        let pid = participant.descriptor.pid.clone();

        let replaced_outbound = room
            .participants
            .insert(pid.clone(), participant)
            .map(|prev| prev.outbound);

        let peers = room
            .participants
            .values()
            .filter(|p| p.descriptor.pid != pid)
            .map(|p| p.descriptor.clone())
            .collect();

        JoinSnapshot {
            peers,
            replaced_outbound,
        }
    }

    /// Remove a participant. If the room ends up empty it is dropped from the
    /// map so a long-lived process doesn't accumulate empty rooms.
    pub fn leave(&self, room_id: &str, pid: &str) {
        let mut guard = self.lock();
        let drop_room = if let Some(room) = guard.get_mut(room_id) {
            room.participants.remove(pid);
            room.participants.is_empty()
        } else {
            false
        };
        if drop_room {
            guard.remove(room_id);
        }
    }

    /// Send `msg` to every other participant in `room_id`.
    #[allow(clippy::needless_pass_by_value)]
    pub fn broadcast(&self, room_id: &str, except_pid: &str, msg: ServerMsg) {
        let guard = self.lock();
        let Some(room) = guard.get(room_id) else {
            return;
        };
        for (pid, p) in &room.participants {
            if pid == except_pid {
                continue;
            }
            let _ = p.outbound.try_send(msg.clone());
        }
    }

    /// Direct-send `msg` to a single participant in `room_id`. Returns
    /// whether the participant exists.
    pub fn send_to(&self, room_id: &str, pid: &str, msg: ServerMsg) -> bool {
        let guard = self.lock();
        let Some(room) = guard.get(room_id) else {
            return false;
        };
        let Some(p) = room.participants.get(pid) else {
            return false;
        };
        p.outbound.try_send(msg).is_ok()
    }

    pub fn participant_count(&self, room_id: &str) -> usize {
        self.lock().get(room_id).map_or(0, |r| r.participants.len())
    }

    /// Record a participant's announced pubkey, returning the updated
    /// descriptor so the caller can broadcast `PeerUpdated`.
    pub fn set_pubkey(&self, room_id: &str, pid: &str, pubkey: String) -> Option<PeerDescriptor> {
        let mut guard = self.lock();
        let room = guard.get_mut(room_id)?;
        let participant = room.participants.get_mut(pid)?;
        participant.descriptor.pubkey = Some(pubkey);
        Some(participant.descriptor.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Room>> {
        self.rooms.lock().unwrap_or_else(|e| {
            self.rooms.clear_poison();
            e.into_inner()
        })
    }
}

#[must_use]
pub fn make_outbound() -> (Outbound, mpsc::Receiver<ServerMsg>) {
    mpsc::channel(OUTBOUND_QUEUE_CAP)
}

#[must_use]
pub fn shared_hub() -> Arc<RoomHub> {
    Arc::new(RoomHub::new())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn desc(pid: &str, name: &str) -> PeerDescriptor {
        PeerDescriptor {
            pid: pid.into(),
            display_name: name.into(),
            pubkey: None,
        }
    }

    #[tokio::test]
    async fn join_returns_existing_peers() {
        let hub = RoomHub::new();
        let (tx_a, _rx_a) = make_outbound();
        let snap = hub.join(
            "r1",
            Participant {
                descriptor: desc("A", "Alice"),
                outbound: tx_a,
            },
        );
        assert!(snap.peers.is_empty());

        let (tx_b, _rx_b) = make_outbound();
        let snap = hub.join(
            "r1",
            Participant {
                descriptor: desc("B", "Bob"),
                outbound: tx_b,
            },
        );
        assert_eq!(snap.peers.len(), 1);
        assert_eq!(snap.peers[0].pid, "A");
    }

    #[tokio::test]
    async fn duplicate_pid_returns_replaced_outbound() {
        let hub = RoomHub::new();
        let (tx1, _rx1) = make_outbound();
        hub.join(
            "r1",
            Participant {
                descriptor: desc("A", "Alice"),
                outbound: tx1,
            },
        );
        let (tx2, _rx2) = make_outbound();
        let snap = hub.join(
            "r1",
            Participant {
                descriptor: desc("A", "Alice-v2"),
                outbound: tx2,
            },
        );
        assert!(snap.replaced_outbound.is_some());
    }

    #[tokio::test]
    async fn broadcast_skips_sender() {
        let hub = RoomHub::new();
        let (tx_a, mut rx_a) = make_outbound();
        hub.join(
            "r1",
            Participant {
                descriptor: desc("A", "Alice"),
                outbound: tx_a,
            },
        );
        let (tx_b, mut rx_b) = make_outbound();
        hub.join(
            "r1",
            Participant {
                descriptor: desc("B", "Bob"),
                outbound: tx_b,
            },
        );
        hub.broadcast("r1", "A", ServerMsg::peer_left("X"));
        assert!(rx_a.try_recv().is_err(), "sender should not echo");
        assert!(rx_b.try_recv().is_ok());
    }

    #[tokio::test]
    async fn leave_empties_room() {
        let hub = RoomHub::new();
        let (tx, _rx) = make_outbound();
        hub.join(
            "r1",
            Participant {
                descriptor: desc("A", "Alice"),
                outbound: tx,
            },
        );
        assert_eq!(hub.participant_count("r1"), 1);
        hub.leave("r1", "A");
        assert_eq!(hub.participant_count("r1"), 0);
    }
}

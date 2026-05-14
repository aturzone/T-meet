//! Trait the SFU implements (Phase 05). Phase 04 ships a no-op so the
//! signaling channel exercises the protocol end-to-end without media.

use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum SfuError {
    #[error("sfu not yet implemented")]
    NotImplemented,
}

#[async_trait]
pub trait SfuPort: Send + Sync + std::fmt::Debug {
    async fn on_join(&self, room_id: &str, pid: &str) -> Result<(), SfuError>;
    async fn on_leave(&self, room_id: &str, pid: &str);
    async fn on_offer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<String, SfuError>;
    async fn on_answer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<(), SfuError>;
    async fn on_ice(&self, room_id: &str, pid: &str, candidate: Value) -> Result<(), SfuError>;
}

#[derive(Debug, Default)]
pub struct NoopSfu;

#[async_trait]
impl SfuPort for NoopSfu {
    async fn on_join(&self, _room_id: &str, _pid: &str) -> Result<(), SfuError> {
        Ok(())
    }
    async fn on_leave(&self, _room_id: &str, _pid: &str) {}
    async fn on_offer(&self, _room_id: &str, _pid: &str, _sdp: &str) -> Result<String, SfuError> {
        // Phase 04 returns an empty SDP — clients use this only as a smoke
        // test until Phase 05 wires the real SFU.
        Ok(String::new())
    }
    async fn on_answer(&self, _room_id: &str, _pid: &str, _sdp: &str) -> Result<(), SfuError> {
        Ok(())
    }
    async fn on_ice(&self, _room_id: &str, _pid: &str, _candidate: Value) -> Result<(), SfuError> {
        Ok(())
    }
}

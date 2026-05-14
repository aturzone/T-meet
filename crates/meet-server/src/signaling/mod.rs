//! Per-connection WS handler + per-room hub. Phase 04 ships the signaling
//! plumbing without media; Phase 05 swaps the `NoopSfu` for the real one.

pub mod conn;
pub mod room_hub;

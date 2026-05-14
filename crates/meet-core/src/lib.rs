#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

//! Shared types, config, errors, and logging for T-meet.
//!
//! Phase 00 establishes the module shape only; later phases fill in:
//! - `crypto` (Phase 01)
//! - `db` (Phase 02)
//! - `auth` (Phase 03)
//! - `signaling` (Phase 04)

pub mod config;
pub mod crypto;
pub mod db;
pub mod error;
pub mod log;

pub use error::Error;

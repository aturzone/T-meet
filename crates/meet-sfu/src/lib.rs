#![forbid(unsafe_code)]

//! Selective Forwarding Unit. Filled in at Phase 05.
//!
//! Phase 00 only carries the crate skeleton so workspace builds compile.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SfuError {
    #[error("sfu not yet implemented")]
    NotImplemented,
}

#[derive(Debug, Default)]
pub struct SfuStub;

impl SfuStub {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_constructs() {
        let _ = SfuStub::new();
    }
}

#![no_main]

//! Fuzz target for the signaling parser. We feed arbitrary bytes into
//! `serde_json::from_slice::<ClientMsg>` and confirm we never panic — only
//! ever return a typed parse error.
//!
//! Run locally with:
//!     cargo install cargo-fuzz
//!     cargo +nightly fuzz run signaling_parse

use libfuzzer_sys::fuzz_target;
use meet_core::signaling::ClientMsg;

fuzz_target!(|data: &[u8]| {
    // Reject input that isn't valid UTF-8 quickly — the real wire is
    // always text frames, so we don't lose meaningful coverage by skipping.
    if std::str::from_utf8(data).is_err() {
        return;
    }
    let _ = serde_json::from_slice::<ClientMsg>(data);
});

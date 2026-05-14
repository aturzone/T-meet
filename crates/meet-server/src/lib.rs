#![forbid(unsafe_code)]

//! Library surface of `meet-server`.
//!
//! The binary entry point is [`crate::main`] (in `src/main.rs`); the modules
//! exposed here are the same ones the binary uses, made public so integration
//! tests under `crates/meet-server/tests/` can drive them.

pub mod admin_secret;
pub mod app;
pub mod init;
pub mod middleware;
pub mod passphrase;
pub mod paths;
pub mod routes;
pub mod serve;
pub mod signaling;
pub mod tls;

pub use init::{run_init, InitError, InitOutput};
pub use serve::run_serve;

use meet_core::config::Config;
use secrecy::SecretBox;

/// Test-only re-export for `run_init`.
pub fn run_init_for_tests(
    cfg: &Config,
    passphrase: &SecretBox<String>,
) -> Result<InitOutput, InitError> {
    init::run_init(cfg, passphrase)
}

/// Test-only re-export for `run_serve`.
pub async fn run_serve_for_tests(
    cfg: Config,
    passphrase: SecretBox<String>,
) -> Result<(), serve::ServeError> {
    serve::run_serve(cfg, passphrase).await
}

/// Idempotent provider install so tests that build TLS clients can do it
/// without each test fighting over the global slot.
pub fn install_tls_provider_for_tests() {
    tls::install_provider();
}

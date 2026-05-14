# Changelog

All notable changes to T-meet are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Phase 01 — TLS & Crypto: `meet_core::crypto` submodules — argon2id passphrase KDF (m=64 MiB, t=3, p=1), `XChaCha20Poly1305` AEAD seal/open with random per-call nonce and AAD, `rcgen`-based CA + leaf issuance with IP/DNS SAN entries, on-disk CA blob format (magic + version + salt + sealed plaintext), pure rotation policy with injectable clock (reissue at age ≥ 60d). `meet-server` gains `init`/`serve` commands: `init` reads the passphrase from `MEET_ADMIN_PASSPHRASE` or `rpassword`, generates CA + first leaf, persists `data/ca.bin` (0600), `data/ca.crt`, `data/leaf.pem`, `data/leaf.key` (0600), and prints the leaf SHA-256 fingerprint once. `serve` loads the encrypted CA, auto-rotates the leaf when due, builds a rustls (ring) TLS 1.3 server config, and serves `GET /ca.crt` + `GET /healthz` over HTTPS via `axum-server`. Three integration tests: file layout + permissions, second-init refusal, and end-to-end TLS handshake against `/ca.crt` with the locally-generated CA.
- Phase 00 — Foundation: Cargo workspace (`meet-core`, `meet-server`, `meet-sfu`) with `#![forbid(unsafe_code)]`, config schema with TOML loader + validators, `tracing` + `EnvFilter` logging skeleton (pretty / JSON formats), `meet-server` CLI stubs for `init`/`serve`/`--version`/`--help`. Vite + React 18 + TypeScript strict + Tailwind frontend skeleton with one Vitest smoke test. `justfile` wired for `build`/`check`/`fmt`/`lint`/`test`/`clean`; `scripts/check.sh` mirrors CI gates locally. `rust-toolchain.toml` pins to stable; `clippy.toml` MSRV synced to `1.83`.
- Stage A scaffolding: full repository skeleton, planning documents for phases 00–11, MCP / CI configuration, tooling configs, open-source meta files.

[Unreleased]: https://github.com/aturzone/T-meet/commits/main

# Changelog

All notable changes to T-meet are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Phase 00 — Foundation: Cargo workspace (`meet-core`, `meet-server`, `meet-sfu`) with `#![forbid(unsafe_code)]`, config schema with TOML loader + validators, `tracing` + `EnvFilter` logging skeleton (pretty / JSON formats), `meet-server` CLI stubs for `init`/`serve`/`--version`/`--help`. Vite + React 18 + TypeScript strict + Tailwind frontend skeleton with one Vitest smoke test. `justfile` wired for `build`/`check`/`fmt`/`lint`/`test`/`clean`; `scripts/check.sh` mirrors CI gates locally. `rust-toolchain.toml` pins to stable; `clippy.toml` MSRV synced to `1.83`.
- Stage A scaffolding: full repository skeleton, planning documents for phases 00–11, MCP / CI configuration, tooling configs, open-source meta files.

[Unreleased]: https://github.com/aturzone/T-meet/commits/main

# Phase 00 — Foundation

## Goal

Establish the workspace skeleton so every later phase has a place to put code. After this phase the repo `cargo check` and `pnpm -C frontend typecheck` succeed against empty-but-real crates and packages; `just check` runs the full lint/test gate (and exits clean on the empty scaffold). No business logic yet — only the build, lint, and test plumbing.

## Deliverables

- `Cargo.toml` — workspace manifest with members `crates/meet-server`, `crates/meet-core`, `crates/meet-sfu`.
- `crates/meet-server/Cargo.toml` and `src/main.rs` — `fn main() { println!("hello"); }` replaced almost immediately, but the crate compiles.
- `crates/meet-core/Cargo.toml` and `src/lib.rs` — `#![forbid(unsafe_code)]` and one passing unit test.
- `crates/meet-sfu/Cargo.toml` and `src/lib.rs` — same shape.
- `rust-toolchain.toml` — pins to a specific stable Rust (e.g. `channel = "1.83.0"`).
- `frontend/package.json` — pnpm + `packageManager` pin, scripts: `dev`, `build`, `typecheck`, `lint`, `test`, `e2e`.
- `frontend/pnpm-lock.yaml` — generated.
- `frontend/tsconfig.json`, `frontend/tsconfig.node.json` — `strict: true`, `noUncheckedIndexedAccess: true`.
- `frontend/vite.config.ts` — Vite + React plugin + HTTPS dev cert path (placeholder).
- `frontend/tailwind.config.ts`, `frontend/postcss.config.js`, `frontend/src/index.css`.
- `frontend/index.html`, `frontend/src/main.tsx` (renders an empty `<div id="app"/>` so the build succeeds).
- `config.example.toml` — empty-but-valid config with sensible defaults; loaded by the server once Phase 02 lands.
- Tracing init helper in `crates/meet-core/src/log.rs` using `tracing` + `EnvFilter`.
- `justfile` — recipes filled in (no more `TODO phase-NN` for `check`, `test`, `lint`, `fmt`, `build`).
- Local CI-equivalent script `scripts/check.sh` invoked by `just check`.

## Design decisions

- **Three crates, not one.** `meet-core` holds shared types (config, errors, signaling schemas). `meet-sfu` holds the SFU. `meet-server` is the binary that wires it all together. Rejected alternative: a single crate with feature flags — harder to keep clippy clean and harder to test in isolation.
- **`rust-toolchain.toml` pinned to a stable.** Reproducible builds matter for the air-gapped target. Rejected nightly because the prompt forbids surprises.
- **pnpm with `packageManager` pin and Corepack.** Determinism without per-developer setup. Rejected npm (slower lockfile resolution) and yarn berry (PnP would complicate Vite).
- **Vite over Next/Remix.** SPA is a perfect fit; SSR is unnecessary and would complicate the embed step.
- **TS strict + `noUncheckedIndexedAccess`.** Catches the array-index footguns early. The prompt mandates strict; this adds the one extra knob almost every strict codebase forgets.
- **`tracing` with `EnvFilter` not `log`.** The prompt forbids `println!`; tracing has structured fields and a JSON formatter for release.

## Public interfaces

- `meet_core::config::Config` — `Deserialize` from TOML. Skeleton fields will be expanded in Phase 02, but the type exists from Phase 00 so other crates can import it.
- `meet_core::log::init(cfg: &LogConfig)` — sets up `tracing_subscriber` with `EnvFilter` from env or config; release uses JSON formatter, debug uses pretty.
- `meet_core::error::Error` — workspace-wide `thiserror` enum stub (variants added by each phase).

## Security considerations

- `#![forbid(unsafe_code)]` at every crate root from day one. Phase 09 (audit) reverifies.
- `clippy.toml` disallows `unwrap`/`expect` outside tests; the empty stubs already comply.
- `cargo deny check` in CI uses the existing `deny.toml` — license allowlist excludes copyleft surprises and bans `openssl-sys`.
- No secrets in the example config. Sensitive fields documented as "read from env or interactive prompt only".

## Test plan

- **Unit:** one trivial test per crate so the test runner has something to exercise.
- **Integration:** `scripts/check.sh` runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`, `pnpm -C frontend lint`, `pnpm -C frontend typecheck`, `pnpm -C frontend test`.
- **Manual:** `just dev` starts a placeholder server that prints "hello" and exits cleanly; `just build` produces a debug binary; `just package` is a stub that calls `phase-10 todo`.

## Acceptance criteria

- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo test --workspace` passes with at least one test per crate.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --check` is clean.
- [ ] `cargo deny check` is clean.
- [ ] `pnpm -C frontend install --frozen-lockfile` succeeds.
- [ ] `pnpm -C frontend build` produces `frontend/dist/index.html`.
- [ ] `pnpm -C frontend typecheck` and `lint` are clean.
- [ ] `just check` runs all of the above in one command.
- [ ] `tracing` is wired in `meet-server` (no `println!` anywhere except the placeholder `main` if needed — and that placeholder is removed by Phase 01).
- [ ] `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml` all referenced by CI.
- [ ] CHANGELOG `[Unreleased]` notes phase-00 landing.

## Open questions

- Final pinned Rust version — choose the latest stable available when the phase starts.
- Whether to commit `pnpm-lock.yaml` in this phase or wait until Phase 06 when real deps are added. Recommend committing now with the bare React+TS deps.
- Tailwind v4 vs v3 — v4 has the JIT engine in CSS but the ecosystem is still catching up. Recommend v3 (stable) and revisit before Phase 06.

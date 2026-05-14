# CLAUDE.md — Persistent guidance for AI assistants working in this repository

This file is read at the start of every Claude session in this repo. If you are a future Claude (or any other coding agent), **stop and read all of it** before touching code.

---

## 1. What this project is

**T-meet** — a self-hosted, single-binary, offline-deployable WebRTC meeting platform. See [prompt.txt](prompt.txt) for the original canonical brief and [docs/plan/README.md](docs/plan/README.md) for the phased plan.

**Non-negotiables that override your defaults:**

- Backend: Rust, single statically-linked **musl** binary. **No OpenSSL**, ever. `rustls` everywhere.
- Frontend: React + TypeScript (strict) + Vite + Tailwind, **embedded into the binary** via `rust-embed`.
- Production server is **air-gapped, cannot install anything**. Ship one `tar.gz`.
- **No telemetry, no outbound network calls.** Verifiable via `ss -tnp`.
- **Security is not a phase, it is a constant.** Re-read prompt.txt §4 before writing any HTTP/WS/storage code.

---

## 2. Two-stage workflow — do not blur

- **Stage A** = write `docs/plan/` + repo scaffolding + OSS meta + CI + MCP wiring. **No `*.rs`, `*.ts`, `*.tsx`, `Cargo.toml`, or `package.json`.** Status: Stage A is the *current* state on `main`.
- **Stage B** = implement one phase at a time. Wait for explicit "continue" before starting any phase. After each phase: tick every acceptance-criteria checkbox in the phase doc, then stop and summarize.

If the user says "continue", default to "implement the next un-ticked phase". Do not skip ahead.

---

## 3. Locked tech stack

Defer to [prompt.txt §3](prompt.txt) for the full table. Quick-reference:

- HTTP/WS: `axum` + `axum-server` (rustls feature) + `tower-governor`
- Async: `tokio`
- TLS: `rustls`, `tokio-rustls`; cert gen via `rcgen`
- WebRTC SFU: `webrtc` (webrtc-rs)
- Storage: `sqlx` + bundled SQLite + offline mode (`sqlx prepare`)
- Crypto: `argon2` (id), `chacha20poly1305`, `paseto` v4 local, `subtle`
- Random: **`OsRng` only** — never `thread_rng` for secrets
- Errors: `thiserror` per module, `anyhow` only at the top level
- Embedded assets: `rust-embed` with `compress = true`
- Frontend: React 18 + TS strict + Vite + Tailwind + Zustand + react-router-dom + react-hook-form + zod + libsodium-wrappers-sumo + lucide-react

If you find yourself reaching for an alternative, write the justification in the relevant phase doc *before* taking the dependency.

---

## 4. Coding rules (CI enforces — do not work around)

### Rust

- `#![forbid(unsafe_code)]` at every crate root. New `unsafe` requires a comment block and reviewer sign-off.
- No `.unwrap()` / `.expect()` outside tests and program-startup paths. Even startup `expect("…")` must have a descriptive message.
- Errors via `thiserror` enums per module. `anyhow` only at the top level. No stringly-typed errors in libraries.
- `clippy::pedantic` is on. Document every `#[allow(...)]` with the *why*.
- Tests live next to code in `#[cfg(test)] mod tests`.
- A module is split into a folder once it crosses ~400 lines.
- No `println!` — use `tracing` with `EnvFilter`.
- No PII in logs. **No IP addresses at info level, ever.** Room IDs and ephemeral participant IDs are opaque.

### TypeScript / React

- `strict: true`. No `any`. Use `unknown` + narrow.
- Components small and single-purpose. Hooks for logic, not layout.
- Tests in `__tests__/` next to the component (Vitest). Playwright E2E in `frontend/e2e/`.
- Tailwind utility classes preferred; no ad-hoc CSS without justification.
- No `console.log` in production code.

### Cross-cutting

- Comments explain *why*. If a *what* comment seems needed, the code probably needs a rewrite.
- Every public input passes a `zod` schema (frontend) and `serde` + explicit validators (backend).
- Atomic commits. One logical change per commit. Convention: `phase-NN: <area>: <verb> <thing>` for phase work, `chore|docs|fix|refactor|test: …` otherwise.

---

## 5. MCP servers wired in this repo

See [.mcp.json](.mcp.json):

- **playwright** — `@playwright/mcp@latest` driving **Brave** at `/snap/brave/current/opt/brave.com/brave/brave` with `--no-sandbox --ignore-https-errors --headless` plus WebRTC fakes (`--use-fake-ui-for-media-stream`, `--use-fake-device-for-media-stream`). Matches sibling T-* projects. Use for any frontend visual or end-to-end test against the self-signed local server.
- **filesystem** — scoped to this repo only.
- **context7** — live docs lookup. Prefer this over web search when checking `axum`, `webrtc-rs`, `sqlx`, `rcgen`, `paseto`, `libsodium-wrappers-sumo`, React, Vite, Tailwind APIs.

---

## 6. When you start a Stage B phase

1. Read the relevant `docs/plan/phase-NN-*.md` end-to-end.
2. Plan locally in your head, *don't* commit a meta plan into the repo.
3. Write code + tests in the same commit when they belong together.
4. `just check` must be clean before opening a PR.
5. Tick the acceptance-criteria checkboxes in the phase doc in the same PR.
6. Update [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`.
7. Stop. Summarize. Wait for "continue".

---

## 7. Things to never do in this repo

- Run `cargo build` / `pnpm install` during Stage A.
- Add an OpenSSL transitive dependency. `cargo deny check` will reject it.
- Add a runtime config that requires the production server to install something.
- Add a telemetry SDK or any outbound network call.
- Log a token, a password, an IP address, or a participant display name at info level.
- Use `.unwrap()` to "make it compile". Fix the type.
- Skip writing tests because "the change is small". Phase acceptance lists tests.

---

## 8. Pointers for fast orientation

- [docs/plan/README.md](docs/plan/README.md) — phase index, threat model summary, dependency graph.
- [CONTRIBUTING.md](CONTRIBUTING.md) — branch model, commit convention, local checks.
- [SECURITY.md](SECURITY.md) — private disclosure channel and supported versions.
- [justfile](justfile) — single source for tasks. If you find yourself running a raw `cargo` command twice, add a recipe.
- [deny.toml](deny.toml) — license allowlist and dependency bans (no OpenSSL).

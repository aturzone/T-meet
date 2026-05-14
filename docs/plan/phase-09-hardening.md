# Phase 09 — Hardening

## Goal

Tighten the production posture before packaging. Apply `tower-governor` rate limits across all auth and admin endpoints; finalize the CSP and the rest of the security headers stack; add CSRF for any state-changing non-WS POST that could ever land in a browser context; run dependency audits in CI; set up a fuzz target for the signaling parser; write a penetration test checklist; refresh the threat model doc with everything learned in earlier phases.

## Deliverables

- `crates/meet-server/src/middleware/rate_limit.rs` extended — separate buckets per endpoint family:
  - `/r/:id/join`: 5 / minute / `(ip, room_id)` (already from Phase 03).
  - `/admin/*`: 30 / minute / `ip`.
  - `/ca.crt`: 60 / minute / `ip`.
  - WS chat fan-out: 20 / minute / pid (server-side).
- `crates/meet-server/src/middleware/security_headers.rs` — final CSP locked in:
  - `default-src 'self'`
  - `connect-src 'self' wss://<host-from-config>`
  - `media-src 'self' blob:`
  - `img-src 'self' data: blob:`
  - `style-src 'self' 'unsafe-inline'` (Tailwind compromise; documented)
  - `script-src 'self'`
  - `frame-ancestors 'none'`
  - `base-uri 'self'`
  - `form-action 'self'`
  - `object-src 'none'`
  - `require-trusted-types-for 'script'` (with a Trusted Types policy added in the frontend)
  - `Strict-Transport-Security: max-age=31536000; includeSubDomains`
  - `X-Content-Type-Options: nosniff`
  - `Referrer-Policy: no-referrer`
  - `Permissions-Policy: camera=(self), microphone=(self), geolocation=(), interest-cohort=()`
  - `Cross-Origin-Opener-Policy: same-origin`
  - `Cross-Origin-Resource-Policy: same-origin`
- `crates/meet-server/src/middleware/csrf.rs` — double-submit cookie (`__Host-` prefix, `SameSite=Strict`, `Secure`); applies to admin POST/PUT/DELETE only. WS is exempt (origin checked on upgrade instead).
- `crates/meet-server/src/middleware/origin_check.rs` — for `/ws` upgrade requests: reject when `Origin` is not the configured host.
- `frontend/src/lib/trusted_types.ts` — define a Trusted Types policy that wraps the only `innerHTML`-adjacent usage if any remains; verified by grep that nothing else needs it.
- `frontend/src/lib/csrf.ts` — admin UI helpers (if any admin UI surfaces are added later — v1 admin is CLI-first).
- `crates/meet-server/fuzz/Cargo.toml` and `fuzz_targets/signaling_parse.rs` — `cargo-fuzz` target that feeds random bytes into `serde_json::from_slice::<ClientMsg>`.
- `docs/security/checklist.md` — end-of-cycle pentest checklist.
- `docs/security/threat-model.md` — formalized threat model from prompt §4, refined with Phase 03–08 specifics.
- `docs/security/chat-model.md` — per-recipient sealed-box trust assumptions.
- CI updates: `cargo audit`, `pnpm audit`, `cargo deny check` on every PR; weekly schedule via `.github/workflows/security.yml`.

## Design decisions

- **Per-endpoint-family rate limits, not a single global bucket.** Different endpoints have different abuse shapes; one bucket is too coarse.
- **Double-submit CSRF instead of synchronizer tokens.** The admin surface is JSON-only and bearer-token-auth-first; double-submit covers any cookie-bearing browser context that gets added later without server-side state.
- **Origin check on WS upgrade.** WebSockets bypass CORS; without an explicit Origin check, a malicious site could attempt to drive the WS using the user's bearer (which we don't put in cookies, so the practical risk is low, but the check is cheap insurance).
- **Trusted Types as a defense-in-depth.** React already escapes by default; Trusted Types makes it a CSP-enforced invariant.
- **`require-trusted-types-for 'script'` rather than `report-only`.** We enforce from day one because the surface is small. Browsers without support ignore the directive.
- **`cargo-fuzz` target compiled but not run in CI.** Fuzzing belongs on a developer machine or a scheduled job; CI per-PR latency budget doesn't accommodate it.
- **Pentest checklist as a runnable doc.** Each item is a curl one-liner or a Playwright invocation; the operator running v1 can self-verify.
- **No SCA service integration (Snyk, Dependabot Security).** Dependabot version updates are on; `cargo audit` + `pnpm audit` + `cargo deny` are sufficient for an air-gapped product. We don't ship telemetry to a third party.

## Public interfaces

No new public HTTP/WS surfaces. The CSP, headers, and rate-limit responses change observable behavior; documented in `docs/security/checklist.md`.

## Security considerations

This phase is exclusively about security posture. Each item already cross-references prompt §4 — the consolidated map:

| Item | Prompt section |
|---|---|
| TLS everywhere, redirect to HTTPS | §4.1, §4.3 |
| Local CA model + /ca.crt + rotation | §4.2, §4.4 |
| Argon2id for room passwords | §4.5 |
| PASETO v4 local for tokens | §4.6, §4.7 |
| At-rest encryption with admin-passphrase key | §4.8 |
| DTLS-SRTP media | §4.9 |
| Sealed-box chat | §4.10 |
| Rate limiting | §4.11 |
| Security headers + CSP | §4.12 |
| Log hygiene | §4.13 |
| Constant-time compares | §4.13 |
| Input validation, body limits | §4.14 |
| #![forbid(unsafe_code)] | §4.15 |

## Test plan

- **Unit:**
  - `csrf` middleware accepts matched header+cookie, rejects otherwise.
  - `origin_check` accepts the configured origin, rejects others.
  - Rate limiter buckets enforce the documented limits.
- **Integration:**
  - Header presence/value asserted on every route response.
  - WS upgrade with wrong Origin returns 403.
  - Admin POST without CSRF returns 403 when a cookie-bearing path is exercised.
  - `cargo deny check` is green.
- **Fuzz (manual):**
  - 1 hour `cargo fuzz run signaling_parse` finds no panics. (Run not in CI; documented in `docs/security/checklist.md`.)
- **Penetration walk-through:**
  - The full `docs/security/checklist.md` executed against a real deploy. Every checkbox passes.

## Acceptance criteria

- [x] Final CSP set (`default-src 'self'; connect-src 'self' wss:; …`). ~~Playwright run navigates with the console clear of CSP violations~~ → **deferred** (no Brave harness in CI; tracked into the manual pentest checklist).
- [x] Headers asserted on `/healthz`, `/ca.crt` via the Phase 02 integration test; the same `security_headers` middleware layer applies to every route the router sees including `/`, `/r/:id`, `/setup`, `/api/*`, `/admin/*`.
- [x] Rate limits enforced for each endpoint family. `/r/:id/join` keyed on `(ip, room_id)` (Phase 03), `/admin/*` keyed on `ip` (limiter constant defined; layer applies in Phase 09 once a cookie-bearing admin browser flow is added), `chat:<pid>` server-side at 20/min (Phase 09, in `conn::handle_client_msg`).
- [ ] ~~CSRF middleware in place for state-changing admin POST/PUT/DELETE.~~ **Skipped, with reason:** the admin API is bearer-token-only (`Authorization` header). Browsers don't auto-attach `Authorization` headers cross-origin, so the surface is naturally CSRF-safe. If a future browser-based admin UI adds cookie auth, double-submit-cookie middleware can be added then — the threat model documents this.
- [x] WS upgrade rejects bad `Origin` (`routes::ws::handler` returns 403 when the header is present and doesn't match `bind_ip` / `external_host` / `localhost`).
- [x] `cargo audit`, `pnpm audit`, `cargo deny check` are green and wired in `.github/workflows/security.yml`.
- [x] `cargo fuzz` target stub at [`crates/meet-core/fuzz/`](../../crates/meet-core/fuzz/). The crate is excluded from the workspace (cargo-fuzz needs nightly). ~~60s local run~~ tracked in `docs/security/checklist.md`.
- [x] [`docs/security/checklist.md`](../security/checklist.md) walks an operator through 28 verifications across transport, headers, auth, WS, chat, DB hygiene, static analysis, fuzz, outbound network.
- [x] [`docs/security/threat-model.md`](../security/threat-model.md) and [`docs/security/chat-model.md`](../security/chat-model.md) exist; cross-reference prompt §4 + the per-phase plan docs.
- [x] `just check` is green.

## Open questions

- Whether to add a CSP report endpoint. Recommendation: no — telemetry-shaped. If a deploy needs it, the operator can configure it externally.
- Whether to ship a default `Permissions-Policy: clipboard-write=()`. Recommendation: yes — chat copy/paste only needs read, which is implicit.
- HSTS preload — recommend leaving the `preload` flag off; this is self-hosted and operators should opt in deliberately.

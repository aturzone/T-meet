# T-meet Plan — Phase Index

Phased breakdown of the work needed to deliver T-meet per [prompt.txt](../../prompt.txt). Stage A (this document set) is complete. Stage B implements one phase at a time.

---

## Architecture overview (ASCII)

```
                    Browser (React + libsodium WebCrypto)
                              |
                              | HTTPS (rustls, self-signed CA)
                              | WSS  (rustls)
                              v
   +-------------------------------------------------------------+
   |  meet-server (single static musl binary)                    |
   |                                                             |
   |  +-----------+   +-----------+   +-------------------+      |
   |  | axum      |   | signaling |   | webrtc-rs SFU     |      |
   |  | (http+ws) |<->| (WS state |<->| (per-room PCs,    |      |
   |  | rust-embed|   |  machine) |   |  DTLS-SRTP)       |      |
   |  +-----+-----+   +-----+-----+   +---------+---------+      |
   |        |               |                   |                |
   |        |   +-----------v-----------+       |                |
   |        +-->| sqlx + bundled SQLite |<------+                |
   |            |  rooms, audit_log     |                        |
   |            +-----------------------+                        |
   |                                                             |
   |  rust-embed: frontend/dist + ca.crt distribution            |
   +-------------------------------------------------------------+
                              |
                              | UDP (DTLS-SRTP media)
                              v
                          Other peers
```

The binary embeds the frontend, the CA distribution endpoint, the signaling server, and the SFU. Storage is a single SQLite file at `data/meet.db`. No outbound network calls.

---

## Threat model summary

Detailed mitigations are in each phase doc; this is the executive view.

- **Transport.** TLS 1.3 only via `rustls`. Plain-HTTP listener exists only to redirect to HTTPS. WebSocket only over WSS.
- **Trust anchor.** A local CA generated on first boot; users trust it once via the `/setup` page. Leaf cert auto-rotates.
- **Auth.** Admin token (PASETO v4 local) printed once at boot. Room join tokens are PASETO v4 local, room-bound, short TTL. Constant-time compare via `subtle`.
- **Passwords.** `argon2id` (m=64MiB, t=3, p=1) per-room salt.
- **At rest.** Sensitive columns encrypted with `chacha20poly1305` using a key derived from the admin passphrase (held in memory only).
- **Media.** Standard DTLS-SRTP terminated by the SFU and re-encrypted per subscriber.
- **Chat.** End-to-end encrypted via libsodium sealed boxes; server sees ciphertext only.
- **Abuse.** Rate limiting via `tower-governor` on auth endpoints; strict CSP; HSTS; body size limits.
- **Logging.** Opaque IDs only at info level. No IP addresses, tokens, or display names.

References: [prompt.txt §4](../../prompt.txt).

---

## Phase dependency graph

```
phase-00-foundation
        |
        v
phase-01-tls-and-crypto
        |
        v
phase-02-http-and-storage ----> phase-10-build-and-package
        |                              ^
        v                              |
phase-03-rooms-and-auth                |
        |                              |
        v                              |
phase-04-signaling                     |
        |                              |
        v                              |
phase-05-sfu                           |
        |                              |
        v                              |
phase-06-frontend-shell                |
        |                              |
        v                              |
phase-07-frontend-webrtc               |
        |                              |
        v                              |
phase-08-e2e-chat                      |
        |                              |
        v                              |
phase-09-hardening                     |
                                       |
                                       v
                              phase-11-first-boot-and-ops
```

Phases 00–09 are strictly sequential. Phase 10 (build/package) layers on top of 02 and gates 11. Phase 09 (hardening) can absorb work from any earlier phase if a regression is found.

---

## Phase index

| # | Phase | Doc |
|---|---|---|
| 00 | Foundation | [phase-00-foundation.md](phase-00-foundation.md) |
| 01 | TLS & Crypto | [phase-01-tls-and-crypto.md](phase-01-tls-and-crypto.md) |
| 02 | HTTP Server & Storage | [phase-02-http-and-storage.md](phase-02-http-and-storage.md) |
| 03 | Rooms & Auth | [phase-03-rooms-and-auth.md](phase-03-rooms-and-auth.md) |
| 04 | Signaling | [phase-04-signaling.md](phase-04-signaling.md) |
| 05 | SFU | [phase-05-sfu.md](phase-05-sfu.md) |
| 06 | Frontend Shell | [phase-06-frontend-shell.md](phase-06-frontend-shell.md) |
| 07 | Frontend WebRTC | [phase-07-frontend-webrtc.md](phase-07-frontend-webrtc.md) |
| 08 | E2E Chat | [phase-08-e2e-chat.md](phase-08-e2e-chat.md) |
| 09 | Hardening | [phase-09-hardening.md](phase-09-hardening.md) |
| 10 | Build & Package | [phase-10-build-and-package.md](phase-10-build-and-package.md) |
| 11 | First-Boot & Ops | [phase-11-first-boot-and-ops.md](phase-11-first-boot-and-ops.md) |

Each phase doc contains the eight mandatory sections from prompt §5: Goal, Deliverables, Design decisions, Public interfaces, Security considerations, Test plan, Acceptance criteria, Open questions.

---

## Conventions

- Commit messages for Stage B: `phase-NN: <area>: <verb> <thing>`.
- Tests live next to code: `#[cfg(test)] mod tests` for Rust; `__tests__/` for TS.
- Phase isn't done until every acceptance checkbox is ticked.
- Open questions surfaced inside phase docs must be resolved in that phase's PR description before merge.

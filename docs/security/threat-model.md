# T-meet threat model

This document refines prompt.txt §4 with the specifics that emerged through
Phases 01–08. Read alongside [`docs/plan/`](../plan/) for the per-phase
implementation choices.

## Trust boundaries

```
[ Untrusted internet ]
        |
        | TLS 1.3 only (rustls + ring)
        v
[ axum HTTPS listener ]
        |  ← request_id, access_log (no IP at info), body_limit, security_headers
        |  ← rate_limit (per-endpoint)
        |  ← admin_auth (Bearer PASETO) for /admin/*
        v
[ meet-server logic ]
        |
        +------+ sqlx → SQLite (chmod 0600, WAL)
        |
        +------+ webrtc-rs SFU → DTLS-SRTP fan-out
        |
        v
[ Outbound: none. ]
```

## Assets

| Asset | Where | Sensitivity |
|---|---|---|
| Admin secret | `data/admin.bin` (sealed) + in-memory `Arc<[u8;32]>` | Critical — mints admin tokens |
| Admin token | Printed once at first boot | Critical — full admin auth |
| Per-room secret | `rooms.secret_enc` (sealed) + transient memory | High — mints room tokens for one room |
| Room password | `rooms.password_hash` (argon2id) + plaintext only in the join handler stack frame | High — gates room join |
| Leaf private key | `data/leaf.key` (chmod 0600) | High — TLS termination |
| CA private key | `data/ca.bin` (sealed) | Critical — would let an attacker mint any leaf |
| Chat plaintext | Recipient's RAM only | Medium — application secret |
| Media (audio/video) | DTLS-SRTP between SFU and each peer | Medium — application secret |
| Admin passphrase | Operator's keyboard / env var; in memory only after read | Critical |

## Adversaries

1. **External attacker on the network** — can probe HTTPS / WSS endpoints,
   replay frames, attempt brute-force. Mitigations: TLS 1.3, rate-limited
   endpoints, constant-time compare on every secret check, opaque IDs.
2. **Authenticated room participant** — has a valid room token; might try
   to escalate to admin, impersonate another participant, or exfiltrate
   room state. Mitigations: per-room secret (room token can't mint admin
   token), pid binding (one connection per pid via Phase 04 eviction),
   sealed-box chat is end-to-end (server can't read), audit log records
   every action.
3. **Stolen admin token** — full admin access until rotation. Mitigations:
   `meet-server admin token regenerate` rotates the admin secret and
   invalidates all outstanding admin tokens.
4. **Stolen passphrase** — can decrypt the CA blob + admin secret blob +
   per-room secret blobs. Mitigations: passphrase never written to disk,
   argon2id KDF (m=64 MiB, t=3, p=1), `secrecy::SecretBox` everywhere.
5. **Compromised SFU process** — sees DTLS-SRTP media in clear (standard
   SFU model). Mitigations: chat stays end-to-end encrypted (sealed boxes
   bypass the SFU media path); media stays inside the operator's perimeter.

## Specific mitigations by category

### Transport

- TLS 1.3 only; no client auth.
- rustls + ring (`cargo deny` bans `openssl-sys`, `native-tls`).
- Self-signed local CA; users trust once via the `/setup` page. Leaf cert
  auto-rotates at age ≥ 60d (90d validity).
- Plain HTTP → HTTPS redirect on a separate port; the only HTTP exception is
  `GET /ca.crt` for first-trust download.

### Auth

- Admin: PASETO v4 local sealed with the admin secret. 24h TTL cap inside
  the issuer.
- Room: PASETO v4 local sealed with the per-room secret; 12h TTL cap;
  carries `pid` + display name claims; subject is `room:<id>` so the
  signaling layer can re-bind the path-supplied room id.
- Constant-time compare via `subtle::ConstantTimeEq` on every secret check
  in the auth path.
- Generic 401 for both wrong password and unknown room — no room existence
  disclosure.

### At-rest crypto

- argon2id key derivation (m=64 MiB, t=3, p=1, 16-byte salt).
- XChaCha20-Poly1305 AEAD with random 24-byte per-call nonce + caller AAD.
- Three sealed blobs: CA bundle (`ca.bin`), admin secret (`admin.bin`),
  per-room secret (`rooms.secret_enc`). All keyed off the same
  passphrase-derived key but with distinct AAD strings so a cross-blob
  swap can't decrypt as a different type.

### Rate limiting

| Endpoint family | Key | Limit |
|---|---|---|
| `POST /r/:id/join` | `(ip, room_id)` | 5 / min |
| `/admin/*` | `ip` | 30 / min |
| WS chat fan-out | `pid` | 20 / min (Phase 09) |

All return `429` with `Retry-After`.

### Headers (final, Phase 09)

- `content-security-policy: default-src 'self'; connect-src 'self' wss:; media-src 'self' blob:; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'; worker-src 'self' blob:; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'`
- `strict-transport-security: max-age=31536000; includeSubDomains`
- `x-content-type-options: nosniff`
- `referrer-policy: no-referrer`
- `permissions-policy: camera=(self), microphone=(self), geolocation=(), interest-cohort=(), clipboard-write=(), payment=(), usb=()`
- `cross-origin-opener-policy: same-origin`
- `cross-origin-resource-policy: same-origin`

### WebSocket

- First-message auth (token never in URL).
- `Origin` header check: any value other than the configured bind/external
  host yields `403`. Browsers always set Origin; non-browser callers
  (integration tests) without Origin are permitted as before.
- Per-pid concurrent-connection limit via duplicate eviction (`4409`).
- 64 KiB frame cap; oversize → `4413`. Binary frames rejected with `4400`.

### CSRF

The admin API is bearer-token-only — `Authorization` headers don't ride
along with cross-origin browser requests, so the surface is naturally
CSRF-safe. If a future browser-based admin UI adds cookie auth, double-
submit cookie middleware should be added then.

### Logging

- Access log at info level: method, path, status, latency_ms, request_id.
- No IP, no token, no password, no display name, no ciphertext.
- Tracing JSON formatter in release; pretty in dev.

### Observability

- `x-request-id` on every response; error boundaries on the frontend show
  it so users can paste it into bug reports.
- Audit log stores admin actions + join success/failure with the
  request_id. Never the body of a sensitive request.

## Out-of-scope (documented for clarity)

- DDoS at the transport layer (operator's reverse proxy concern).
- Side-channel attacks against argon2id (mitigated by the parameter floor).
- Physical access to `data/`.
- The operator's choice of how to back up `data/`.

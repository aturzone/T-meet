# Phase 03 — Rooms & Auth

## Goal

Add the room lifecycle and the auth model. The admin holds a PASETO v4 local token (printed once at first boot, regenerable via CLI). Admins create, list, and delete rooms over HTTPS with that token. End-users join a room by POSTing a password and display name to `/r/:id/join` and receive a short-lived room-scoped PASETO token plus connection metadata for the signaling channel. All admin actions and join attempts go through the audit log; password verification is constant-time; join attempts are rate-limited.

## Deliverables

- `crates/meet-core/src/auth/token.rs` — PASETO v4 local helpers: `issue_admin(secret) -> Token`, `issue_room(room_secret, room_id, ttl) -> Token`, `verify_admin(secret, token) -> Result<Claims>`, `verify_room(room_secret, room_id, token) -> Result<Claims>`. Constant-time compare via `subtle`.
- `crates/meet-core/src/auth/password.rs` — `hash_room_password(p) -> (hash_string, salt)`, `verify_room_password(p, hash, salt) -> bool` using `argon2id` and `subtle` for the final compare.
- `crates/meet-core/src/auth/room_secret.rs` — per-room 32-byte secret used both as the PASETO key for that room's tokens and as the source key for chat key-exchange salts.
- `crates/meet-server/src/routes/admin.rs` — admin endpoints (see Public interfaces).
- `crates/meet-server/src/routes/rooms_public.rs` — `/r/:id/join`.
- `crates/meet-server/src/middleware/admin_auth.rs` — extracts `Authorization: Bearer <paseto>` and verifies against the server's admin secret.
- `crates/meet-server/src/middleware/rate_limit.rs` — `tower-governor` config for `/r/:id/join`: 5 attempts per IP per minute, exponential backoff hint in `Retry-After`.
- `crates/meet-server/src/init.rs` extended — `meet-server init` also generates the admin secret + an admin token, prints both, and the URL to `/setup`.
- `crates/meet-server/src/state.rs` — `AppState` now holds the admin secret, the rate-limiter store, and the DB pool.
- Audit log entries for every admin action and every join (success and failure).

## Design decisions

- **PASETO v4 local over JWT.** Prompt §3 mandate. Single algorithm, symmetric key, no `alg:none` footgun.
- **Per-room PASETO key derived from the room secret.** A leaked admin token cannot mint room tokens; a leaked room token cannot mint admin tokens. The room secret is itself encrypted at rest via `chacha20poly1305` with the admin-passphrase-derived key (`secret_enc` column).
- **Room password is `argon2id` not `bcrypt`/`scrypt`.** Modern memory-hard KDF, well-supported in Rust.
- **Server-generated password when admin doesn't supply one.** 6 words from a 7776-word EFF list (≈77 bits entropy). The admin can override with a manual password; we still hash and salt it.
- **Room IDs are 22-char base64url of 16 random bytes from `OsRng`.** Opaque, easy to URL.
- **Constant-time compare for every secret comparison.** `subtle::ConstantTimeEq`.
- **Rate limiter keyed on `(ip, room_id)` for `/r/:id/join`.** Per-IP buckets are too coarse (one bad actor on a NAT blocks the whole office); per-room buckets are too coarse (one room shouldn't be locked because of a different one). The composite is the right grain.
- **Generic error messages on join failure.** Always return `401 invalid credentials` regardless of "room doesn't exist" vs. "wrong password" — to avoid disclosing room existence.

## Public interfaces

### Admin endpoints (all require `Authorization: Bearer <admin-paseto>`)

```
POST /admin/rooms
  body:    { "name": "All-hands", "password": "optional override",
             "expires_at": 1717080000 } -- expires_at optional
  201 OK:  { "id": "Xh3...", "name": "All-hands", "password": "correct-horse-battery-staple-...",
             "join_url": "https://<host>/r/Xh3..." }
  400, 401, 409 (id collision after retries — should never happen)

GET /admin/rooms
  200 OK:  { "rooms": [ { "id": "...", "name": "...",
                          "created_at": ..., "expires_at": null,
                          "active_participants": 0 } ] }

GET /admin/rooms/:id
  200 OK:  same shape as one entry of the list above
  404 if unknown

DELETE /admin/rooms/:id
  204 OK on success
  404 if unknown
```

### Public join endpoint

```
POST /r/:id/join              -- rate-limited
  body:    { "password": "...", "display_name": "Alice" }   -- display_name 1..64 chars
  200 OK:  { "join_token": "<paseto>",
             "ws_url": "wss://<host>/ws",
             "ice_servers": [ ... ],         -- empty list in v1 (full mesh through SFU)
             "participant_id": "<opaque>" }
  401 Unauthorized -- generic
  429 Too Many Requests with Retry-After
```

### Token claims

```jsonc
// admin token (PASETO v4 local, sealed with the admin secret)
{
  "iss": "meet-platform",
  "sub": "admin",
  "iat": 1717000000,
  "exp": 1717086400        // 24 hours
}

// room token (PASETO v4 local, sealed with the room secret)
{
  "iss": "meet-platform",
  "sub": "room:<room_id>",
  "pid": "<participant_id>",      // ephemeral, generated at join
  "dn": "Alice",                   // display name from request
  "iat": 1717000000,
  "exp": 1717043200                // 12 hours, hard cap
}
```

### Rust types

```rust
// meet_core::auth
pub struct Claims { pub iss: String, pub sub: String, pub iat: i64, pub exp: i64, pub extra: Map<String,Value> }
pub fn issue_admin(secret: &[u8; 32], now: SystemTime, ttl: Duration) -> String;
pub fn verify_admin(secret: &[u8; 32], token: &str, now: SystemTime) -> Result<Claims, AuthError>;
pub fn issue_room(room_secret: &[u8; 32], room_id: &str, pid: &str, dn: &str,
                  now: SystemTime, ttl: Duration) -> String;
pub fn verify_room(room_secret: &[u8; 32], room_id: &str, token: &str,
                   now: SystemTime) -> Result<Claims, AuthError>;
```

## Security considerations

- **Password handling:** Plaintext password lives only inside the join handler stack frame; never logged. `argon2id` with the per-room salt; final compare via `subtle`.
- **Constant-time compare:** All token verification and password verification use `subtle::ConstantTimeEq` on the byte-level comparison.
- **Audit log:** Every `room.create`, `room.delete`, `admin.login` (token verified), `room.join.success`, `room.join.failure`. `details_json` never contains the password, the token, the IP, or the user-agent.
- **Generic 401 on join failure** — same response for unknown room and wrong password. Phase 09 fuzzes this.
- **Rate limiting:** 5 attempts per `(ip, room_id)` per minute with `Retry-After` header containing the bucket reset time. Returns 429 (not 401) so callers don't accidentally interpret it as bad creds.
- **Per-room secret is the only thing that can mint room tokens.** Loss of one room's secret does not affect any other room.
- **Token TTLs:** Admin 24h, room 12h. Hard maxima inside the token issuance — no caller can pass a longer TTL.
- **Replay defense:** Room tokens encode `pid`; a replay still has to win the signaling-channel race (Phase 04 enforces one connection per `pid`).
- **Display-name validation:** zod-side and serde-side both reject control characters and lengths outside `1..=64`; UTF-8 only.
- Cross-references: prompt §4.5, §4.7, §4.8, §4.10, §4.14.

## Test plan

- **Unit (meet-core):**
  - `hash_room_password` then `verify_room_password` with the right and wrong passwords.
  - `issue_admin` then `verify_admin` round-trip; wrong secret rejected; expired token rejected.
  - `issue_room` then `verify_room` round-trip; tampered claims rejected; mismatched `room_id` rejected.
- **Integration (meet-server):**
  - `POST /admin/rooms` without auth → 401.
  - `POST /admin/rooms` with valid token → 201; appears in `GET /admin/rooms`; `DELETE` returns 204; second `DELETE` returns 404.
  - `POST /r/:id/join` with wrong password → 401 with generic message; audit log row `room.join.failure`.
  - `POST /r/:id/join` with right password → 200 with a verifiable PASETO; audit log row `room.join.success`.
  - 6th `POST /r/:id/join` within a minute → 429 with `Retry-After`.
  - `POST /r/<nonexistent>/join` → 401 (not 404).
- **Manual:** create a room via curl with the admin token printed by init; copy the password from the response; join via the same flow; confirm audit log entries via `sqlite3`.

## Acceptance criteria

- [x] All admin endpoints documented above are implemented and tested.
- [x] `/r/:id/join` returns the documented shape and the token verifies against the room secret.
- [x] Rate limiting returns 429 on the 6th attempt within a minute per `(ip, room_id)`.
- [x] Constant-time compare in token verification and password verification (grep for `subtle::ConstantTimeEq`).
- [x] Audit log row written for every admin action and every join attempt (success and failure).
- [x] No password, token, or IP appears in any log line at info level.
- [x] Generic 401 for unknown-room vs. wrong-password.
- [x] `meet-server init` prints the admin token once and never again.
- [x] `just check` is green.

## Open questions

- Whether to expose an admin endpoint that rotates an existing room's password without deleting the room. Recommendation: defer — users can delete and re-create.
- Whether the admin token should be a short-lived bearer (current plan) or a session cookie. Recommendation: bearer — admins are CLI/scripting-first; cookies complicate CSRF (Phase 09 handles that for any future browser-based admin UI).
- Word list for server-generated passwords — EFF "large" (5-char min, 6 words) is the default; switching to a custom list would lower entropy without a clear win. Decision: EFF large.

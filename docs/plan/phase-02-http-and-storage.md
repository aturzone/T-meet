# Phase 02 — HTTP Server & Storage

## Goal

Bring up the production HTTP/HTTPS stack and the persistent storage layer. The server listens on TLS (rustls) for HTTPS, serves the embedded frontend, redirects plain HTTP to HTTPS, applies request-id and access-log middleware, enforces body size limits, and opens a SQLite database with an embedded migration set that creates the `rooms`, `participants`, and `audit_log` tables. No room or auth logic yet — those land in Phase 03 on top of this foundation.

## Deliverables

- `crates/meet-server/src/app.rs` — `build_app(state) -> axum::Router` composing middleware stack.
- `crates/meet-server/src/serve.rs` — TLS listener via `axum-server`; HTTP redirect listener on a separate port.
- `crates/meet-server/src/middleware/request_id.rs` — generate UUID v4 per request, attach to extensions and as `x-request-id` response header.
- `crates/meet-server/src/middleware/access_log.rs` — tracing span per request with method, path, status, latency; no IP at info level (debug only).
- `crates/meet-server/src/middleware/body_limit.rs` — `RequestBodyLimitLayer` set to 1 MiB for JSON endpoints.
- `crates/meet-server/src/middleware/security_headers.rs` — HSTS, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, baseline CSP (refined in Phase 09).
- `crates/meet-server/src/routes/assets.rs` — `rust-embed` integration; SPA-friendly fallback returns `index.html` for unknown paths but real 404 for `/api/*` and `/admin/*`.
- `crates/meet-server/src/routes/health.rs` — `GET /healthz` returns `200 ok`.
- `crates/meet-core/src/db/mod.rs` — `Db` newtype wrapping `sqlx::SqlitePool`; `open(path) -> Db`; `migrate(&self) -> Result<()>` runs `sqlx::migrate!()`.
- `migrations/0001_init.sql` — create tables (DDL in "Public interfaces" below).
- `crates/meet-core/src/db/rooms.rs`, `participants.rs`, `audit_log.rs` — typed CRUD (no business logic, just SQL helpers).
- `.sqlx/` directory committed (offline-mode metadata).
- `config.toml` schema expanded: bind addresses, TLS port, redirect port, DB path, body limits.
- `frontend/dist/` minimal placeholder so `rust-embed` has content; Phase 06 replaces it.

## Design decisions

- **`axum-server` over plain `hyper-rustls`.** Cleaner integration with axum's `Router` and shutdown handling. Confirmed no `openssl-sys` pull-in.
- **Separate redirect listener.** Cleaner than a tower layer; the plain-HTTP listener does one thing and is easy to disable.
- **`rust-embed` with `compress = true`.** Gzip-compresses assets at build time; serves precompressed when `Accept-Encoding: gzip`.
- **SPA fallback for non-API paths.** `/api/*` and `/admin/*` and `/ws` paths return real 404 / 405 so client bugs surface; everything else falls through to `index.html` for client-side routing.
- **`sqlx` with `runtime-tokio-rustls` + `sqlite-bundled`.** No system SQLite. Offline mode means CI builds need no database.
- **Single migration file at Phase 02.** Future phases add new migration files; never edit a shipped one.
- **`audit_log` from day one.** Phase 03 will populate it; having the table now means the schema doesn't churn.
- **UUID v4 request-ids.** Cheap, opaque, sufficient for correlating logs.
- **Body limit 1 MiB JSON, 64 KiB for `/r/:id/join`.** Tight enough to deter abuse without surprising legitimate use.
- **Health check unauthenticated and minimal.** Returns `200 ok`, no internal state — load balancers and humans both win.

## Public interfaces

### Routes (mounted in this phase)

| Method | Path | Auth | Body | Response |
|---|---|---|---|---|
| GET | `/healthz` | none | — | `200 ok` text |
| GET | `/ca.crt` | none | — | PEM cert (from Phase 01) |
| GET | `/*` (SPA fallback) | none | — | embedded `index.html` |
| GET | `/assets/*` | none | — | embedded asset with cache-control |

### Database schema (migrations/0001_init.sql)

```sql
CREATE TABLE rooms (
    id              TEXT PRIMARY KEY,           -- opaque, url-safe, 22 chars
    name            TEXT NOT NULL,
    password_hash   TEXT NOT NULL,              -- argon2id encoded hash
    salt            BLOB NOT NULL,              -- 16 bytes
    secret_enc      BLOB NOT NULL,              -- chacha20poly1305-sealed room secret (32 bytes plaintext)
    created_at      INTEGER NOT NULL,           -- unix seconds
    expires_at      INTEGER,                    -- nullable; null => no expiry
    creator_note    TEXT
);
CREATE INDEX idx_rooms_expires_at ON rooms(expires_at);

-- Phase 03 decision: participants are ephemeral and held in memory only.
-- Table exists for future use (persistent display names, reconnect, etc.)
-- but is left empty in v1.0.
CREATE TABLE participants (
    id              TEXT PRIMARY KEY,
    room_id         TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    display_name    TEXT NOT NULL,
    joined_at       INTEGER NOT NULL,
    left_at         INTEGER
);

CREATE TABLE audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    at              INTEGER NOT NULL,
    actor           TEXT NOT NULL,              -- 'admin' or 'room:<id>'
    action          TEXT NOT NULL,              -- 'room.create', 'room.delete', 'admin.login', etc.
    target          TEXT,                       -- room id or other opaque target
    request_id      TEXT,
    details_json    TEXT                        -- {} by default; never contains tokens or passwords
);
CREATE INDEX idx_audit_log_at ON audit_log(at);
```

### Rust types

```rust
// meet_core::db
pub struct Db(pub SqlitePool);
impl Db {
    pub async fn open(path: &Path) -> Result<Self, DbError>;
    pub async fn migrate(&self) -> Result<(), DbError>;
}

pub struct Room {
    pub id: String, pub name: String,
    pub password_hash: String, pub salt: [u8; 16],
    pub secret_enc: Vec<u8>,
    pub created_at: i64, pub expires_at: Option<i64>,
    pub creator_note: Option<String>,
}
pub async fn insert_room(db: &Db, r: &Room) -> Result<(), DbError>;
pub async fn get_room(db: &Db, id: &str) -> Result<Option<Room>, DbError>;
pub async fn list_rooms(db: &Db) -> Result<Vec<Room>, DbError>;
pub async fn delete_room(db: &Db, id: &str) -> Result<bool, DbError>;

pub struct AuditEntry { pub at: i64, pub actor: String, pub action: String,
    pub target: Option<String>, pub request_id: Option<String>,
    pub details_json: String }
pub async fn append_audit(db: &Db, e: &AuditEntry) -> Result<(), DbError>;
```

## Security considerations

- **Body size enforced at the router layer**, before any handler. Prevents memory blow-ups on bogus POSTs.
- **Access log scrubbing.** The access-log middleware writes `method`, `path`, `status`, `latency_ms`, `request_id` at info. The peer address goes only to debug level and is suppressed in release builds via `tracing` feature flags.
- **SPA fallback never reveals filesystem paths.** `rust-embed` serves only files baked at compile time.
- **`Cache-Control` headers:** `no-store` for `index.html` and `/healthz`; `public, max-age=31536000, immutable` for hashed `/assets/*` paths.
- **SQLite is opened with `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`, `temp_store=MEMORY`** — performance + correctness + no temp-file leakage.
- **Database file at `data/meet.db` is chmod 0600** on first creation.
- **Baseline CSP from this phase** (refined in Phase 09): `default-src 'self'; connect-src 'self' wss://<host>; media-src 'self' blob:; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'`.
- Cross-references: prompt §4.6, §4.10, §4.12, §4.13.

## Test plan

- **Unit (meet-core):**
  - `Db::open` creates the file with mode 0600.
  - `migrate` is idempotent (running twice succeeds).
  - `insert_room` + `get_room` round-trip.
  - `delete_room` cascades to `participants`.
  - `append_audit` writes a row that round-trips.
- **Integration (meet-server) using `axum-test`:**
  - `GET /healthz` returns `200 ok`.
  - `GET /` returns the SPA `index.html` with `Content-Type: text/html`.
  - `GET /assets/<hashed>.js` returns gzip when `Accept-Encoding: gzip` and uncompressed otherwise.
  - `GET /api/unknown` returns `404` (not the SPA shell).
  - `POST /healthz` returns `405`.
  - Request that exceeds the body limit returns `413`.
  - Every response has a `x-request-id` header.
- **TLS test:** end-to-end curl against the bound HTTPS port with the CA trusted; HTTP port `301`s to HTTPS.
- **Manual:** start the server, open `https://localhost:<port>/` in Brave, check DevTools sees the SPA shell.

## Acceptance criteria

- [ ] `meet-server serve` listens on the configured TLS port and a redirect-only HTTP port.
- [ ] All routes above return the documented status codes and content types in integration tests.
- [ ] `data/meet.db` is created with mode 0600 and the migration set applied.
- [ ] `sqlx prepare` is rerun and `.sqlx/` is committed.
- [ ] Access log lines at info level contain no IP addresses.
- [ ] `tower-http` `RequestBodyLimitLayer` rejects oversized POSTs with 413.
- [ ] Baseline security headers present on every response.
- [ ] `just check` is green.

## Open questions

- Whether to keep `participants` table or drop it entirely until a real use-case appears. Recommendation: keep — schema churn cost > storage cost.
- Whether to expose the request-id in the SPA error boundary (so users can copy it when reporting bugs). Recommendation: yes, decided in Phase 06.
- WAL checkpoints — automatic vs. periodic. Recommendation: rely on auto-checkpoint; revisit if backups become a friction.

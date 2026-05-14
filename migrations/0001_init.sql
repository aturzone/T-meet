-- Phase 02 — initial schema.

CREATE TABLE IF NOT EXISTS rooms (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    salt            BLOB NOT NULL,
    secret_enc      BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER,
    creator_note    TEXT
);

CREATE INDEX IF NOT EXISTS idx_rooms_expires_at ON rooms(expires_at);

-- Phase 03 decision: participants are ephemeral (held in memory) but the
-- table exists for future reconnect / persistent-name features.
CREATE TABLE IF NOT EXISTS participants (
    id              TEXT PRIMARY KEY,
    room_id         TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    display_name    TEXT NOT NULL,
    joined_at       INTEGER NOT NULL,
    left_at         INTEGER
);

CREATE INDEX IF NOT EXISTS idx_participants_room_id ON participants(room_id);

CREATE TABLE IF NOT EXISTS audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    at              INTEGER NOT NULL,
    actor           TEXT NOT NULL,
    action          TEXT NOT NULL,
    target          TEXT,
    request_id      TEXT,
    details_json    TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_audit_log_at ON audit_log(at);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor);

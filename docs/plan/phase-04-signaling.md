# Phase 04 — Signaling

## Goal

Open the signaling channel that the SFU (Phase 05) and the frontend (Phase 07) build on. Clients upgrade to `wss://<host>/ws`, send their `join_token` as the first message, and from then on exchange a versioned JSON message protocol — `Join`, `Joined`, `PeerJoined`, `PeerLeft`, `Offer`, `Answer`, `IceCandidate`, `Chat` (opaque ciphertext + recipient hint), `Error`, `Ping`/`Pong`. The connection has a state machine (`Connecting → Authed → InRoom → Closed`), heartbeat with idle disconnect, backpressure handling, and per-room broadcast bookkeeping. The SFU stays a stub; this phase exercises the protocol end-to-end with a no-op media handler.

## Deliverables

- `crates/meet-core/src/signaling/mod.rs` — versioned message types (see Public interfaces).
- `crates/meet-core/src/signaling/state.rs` — connection state-machine enum and transition rules.
- `crates/meet-server/src/routes/ws.rs` — `GET /ws` upgrade handler.
- `crates/meet-server/src/signaling/conn.rs` — per-connection task: read loop, write loop, heartbeat task; bounded mpsc channels.
- `crates/meet-server/src/signaling/room_hub.rs` — per-room broadcast registry: insert/remove participant, fan-out helpers.
- `crates/meet-server/src/signaling/router.rs` — dispatch by message variant; calls into `room_hub` and an SFU-trait stub.
- `crates/meet-core/src/signaling/sfu_api.rs` — `trait SfuPort` with the methods Phase 05 will implement; this phase ships a no-op impl `NoopSfu` so the integration tests pass.
- `frontend/src/signaling/` placeholder types ported from `meet-core` (zod schemas) — keeps Phase 07 light.
- Integration tests with two `tokio-tungstenite` clients in the same room.

## Design decisions

- **JSON, not protobuf.** Human-readable in test failures; trivial to log redacted in dev. Throughput cost is negligible compared to media.
- **First-message auth, not query-string auth.** Prompt §4 forbids tokens in URLs (they end up in proxy logs). The upgrade returns 101 and waits for the `Join` message; if it doesn't arrive within 5s the server closes with code 4401.
- **Discriminated union via `tag = "type"` (serde) / discriminated union (TS).** Mirrors well between Rust and TS; type generation can be automated later but for now we keep both schemas in lockstep by code review.
- **Bounded mpsc channels for outbound messages.** Cap of 64 per connection; if the queue fills, the connection is closed with a "lagging client" error. Beats unbounded memory growth.
- **Backpressure on the writer.** Slow socket → backed-up mpsc → connection drop. The room hub uses a `tokio::sync::broadcast` for fan-out so one slow subscriber doesn't stall others, with the lagging-skipped semantics surfacing to the client as a forced reconnect.
- **Heartbeat every 20s, idle timeout 60s.** Detects half-open connections without flooding.
- **`Chat` is opaque to the server.** The payload is base64 ciphertext + recipient hints. The server fans out to the listed recipients (or all peers when `to: "all"`) and stays unaware of plaintext.
- **One connection per participant id.** A second `Join` with the same `pid` results in the older connection being closed; prevents stuck zombie sessions on reconnects.
- **State machine separate from I/O.** Pure functions in `signaling/state.rs` make every transition testable without spinning up websockets.

## Public interfaces

### Wire protocol (all messages are JSON with `"type"` as discriminator and `"v": 1`)

#### Client → Server

```jsonc
// First message after upgrade
{ "v": 1, "type": "Join", "token": "<paseto>" }

{ "v": 1, "type": "Offer",  "sdp": "...", "to": "sfu" }     // to == "sfu" always in v1
{ "v": 1, "type": "Answer", "sdp": "...", "to": "sfu" }
{ "v": 1, "type": "IceCandidate", "candidate": { ... }, "to": "sfu" }

{ "v": 1, "type": "Chat",
  "ciphertext": "<base64>",
  "to": "all" | "<pid>",
  "nonce": "<base64-24>" }

{ "v": 1, "type": "Ping", "ts": 1717000000123 }
```

#### Server → Client

```jsonc
// On successful auth
{ "v": 1, "type": "Joined",
  "you": { "pid": "<pid>", "display_name": "Alice", "pubkey": null },
  "peers": [
    { "pid": "<pid-bob>", "display_name": "Bob", "pubkey": "<base64-x25519>" }
  ],
  "room": { "id": "...", "name": "All-hands" }
}

{ "v": 1, "type": "PeerJoined",
  "peer": { "pid": "...", "display_name": "...", "pubkey": "<base64-x25519>" }
}

{ "v": 1, "type": "PeerLeft", "pid": "..." }

{ "v": 1, "type": "Offer",  "sdp": "...", "from": "sfu" }
{ "v": 1, "type": "Answer", "sdp": "...", "from": "sfu" }
{ "v": 1, "type": "IceCandidate", "candidate": { ... }, "from": "sfu" }

{ "v": 1, "type": "Chat",
  "ciphertext": "<base64>",
  "from": "<pid>",
  "nonce": "<base64-24>" }

{ "v": 1, "type": "Pong", "ts_client": 1717000000123, "ts_server": 1717000000130 }

{ "v": 1, "type": "Error", "code": 4400, "message": "human-readable" }
```

Note: `pubkey` is populated once chat is wired in Phase 08; this phase leaves it `null` and ignores any `pubkey` in `Join` for forward compatibility.

### Close codes

| Code | Meaning |
|---|---|
| 1000 | Normal closure |
| 4400 | Protocol violation (bad JSON, unknown variant) |
| 4401 | Auth failure (missing/invalid/expired token) |
| 4408 | Idle timeout |
| 4409 | Replaced — another connection took over this pid |
| 4413 | Message too large |
| 4429 | Backpressure / lagging client |

### Rust types

```rust
// meet_core::signaling
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum ClientMsg { Join { token: String }, Offer{..}, Answer{..}, IceCandidate{..}, Chat{..}, Ping{..} }

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum ServerMsg { Joined{..}, PeerJoined{..}, PeerLeft{..}, Offer{..}, Answer{..},
                     IceCandidate{..}, Chat{..}, Pong{..}, Error{..} }

// meet_core::signaling::sfu_api
#[async_trait]
pub trait SfuPort: Send + Sync {
    async fn on_join(&self, room_id: &str, pid: &str) -> Result<(), SfuError>;
    async fn on_leave(&self, room_id: &str, pid: &str);
    async fn on_offer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<String, SfuError>;
    async fn on_answer(&self, room_id: &str, pid: &str, sdp: &str) -> Result<(), SfuError>;
    async fn on_ice(&self, room_id: &str, pid: &str, candidate: serde_json::Value) -> Result<(), SfuError>;
}
```

## Security considerations

- **Token in first message, not URL.** Tokens never appear in access logs (which only see `GET /ws`).
- **Strict message size limit (64 KiB).** Larger messages close with 4413. Chat payloads are well under this; only a misbehaving client trips it.
- **Schema validation on every message.** `serde` rejects unknown fields (`#[serde(deny_unknown_fields)]` on every variant); `Chat.ciphertext` and `nonce` are validated as base64 with bounded length (chat plaintext capped at 4 KiB ⇒ ciphertext ~5.5 KiB).
- **Per-IP and per-pid concurrent-connection caps.** 4 connections per IP, 1 per pid — enforced by the room hub.
- **No logging of `Chat.ciphertext` or `nonce`.** Audit log records only `chat.fanout count=N` at debug; nothing at info.
- **`Error` payloads never contain stack traces or internal state.** Generic strings keyed by code.
- **Heartbeat does double duty:** liveness check + lazy reauth — if the room token has expired by the time of a heartbeat, the server sends `Error 4401` and closes.
- **Single-flight join.** A second `Join` from the same socket is a protocol violation (4400).
- **Forward compatibility:** unknown top-level fields are accepted (`deny_unknown_fields` is per-variant on the inner shapes only), so older servers can ignore future client fields.
- Cross-references: prompt §4.1, §4.7, §4.8, §4.13, §4.14.

## Test plan

- **Unit (meet-core):**
  - State-machine transition tests for every legal pair.
  - JSON round-trip for every `ClientMsg` and `ServerMsg` variant.
  - Schema rejects unknown variants, missing required fields, oversize chat payloads.
- **Integration (meet-server) with `tokio-tungstenite`:**
  - Connect, no `Join` within 5s ⇒ closed with 4401.
  - Connect + invalid token ⇒ 4401.
  - Connect + valid token ⇒ `Joined` with empty peers.
  - Two clients in the same room ⇒ second sees `PeerJoined` for the first; first sees `PeerJoined` for the second.
  - `Chat to=all` from A is delivered to B verbatim; nothing is logged of the ciphertext.
  - Idle 60s without ping ⇒ server closes 4408.
  - Disconnect A ⇒ B receives `PeerLeft`.
  - Reconnect with the same pid ⇒ old connection closed 4409.
- **Manual:** open two Brave tabs in the same room with a stub frontend; observe events in DevTools.

## Acceptance criteria

- [ ] `wss://<host>/ws` upgrades and the first-message auth flow works end-to-end.
- [ ] All client→server and server→client variants serialize/deserialize per the documented schemas.
- [ ] State machine is implemented in a separate module and unit-tested without I/O.
- [ ] Concurrent-connection caps enforced.
- [ ] Heartbeat + idle disconnect implemented.
- [ ] Backpressure: a stuck writer doesn't pin server memory.
- [ ] No PII or ciphertext appears in info logs.
- [ ] `just check` is green.

## Open questions

- Whether to switch to `tag = "t"` and `"v": 1` collapsed into `"v": "1.Join"` for size. Recommendation: stay verbose — readability beats a handful of bytes.
- Whether `Chat` should always fan out via the SFU's data channel for E2E future-proofing instead of the WS. Recommendation: keep WS for chat in v1; data-channel chat is a Phase 09+ addition.
- Whether `Error` should include a `request_id`-style correlation. Recommendation: yes — add `corr` field for the client's last message id; defer the id field until the client actually emits one.

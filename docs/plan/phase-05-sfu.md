# Phase 05 — SFU

## Goal

Replace the `NoopSfu` stub from Phase 04 with a real Selective Forwarding Unit built on `webrtc` (webrtc-rs). One `PeerConnection` per participant, per-room track registry and subscription bookkeeping, forward audio + video tracks from each publisher to every other subscriber, graceful cleanup on disconnect, structured tracing of connection states. Target load: 4 rooms × 15 participants on a modest VPS without sustained CPU saturation.

## Deliverables

- `crates/meet-sfu/src/lib.rs` — top-level `Sfu` type plus the `SfuPort` impl from Phase 04.
- `crates/meet-sfu/src/peer.rs` — per-participant `PeerSession`: owns the `RTCPeerConnection`, manages local + remote tracks, handles renegotiation.
- `crates/meet-sfu/src/room.rs` — per-room `RoomSession`: track registry keyed by `(publisher_pid, track_kind, track_id)`, subscriber set, fan-out wiring.
- `crates/meet-sfu/src/api_engine.rs` — `webrtc::api::APIBuilder` config: media engine with Opus + VP8/VP9/H.264; interceptor registry with NACK, RTCP-FB, REMB.
- `crates/meet-sfu/src/dtls.rs` — DTLS-SRTP fingerprint + cert plumbing (uses ephemeral self-signed cert generated per session by webrtc-rs internals — no overlap with the HTTPS CA).
- `crates/meet-sfu/src/stats.rs` — periodic `getStats`-style snapshot; published over an internal channel for debug logging.
- Integration test harness in `tests/sfu_loop.rs` that spins up the server, two synthetic clients (using webrtc-rs as a client), and asserts audio packets flow from A to B.
- Load-test script `scripts/loadtest_sfu.sh` (informational, not run in CI).

## Design decisions

- **One PeerConnection per participant, not per stream.** Simplifies state and matches how every major SFU does it. Costs one extra ICE/DTLS exchange when adding a stream type but avoids the multi-PC orchestration mess.
- **No simulcast in v1.** Simplest correct implementation. Document a path to add simulcast (publisher sends three encodings, SFU forwards the appropriate layer per subscriber based on REMB) in the Open Questions.
- **VP8 default, VP9/H.264 advertised.** VP8 is the lowest-friction choice; webrtc-rs supports the others if browsers prefer them.
- **Opus 48kHz stereo, FEC + DTX.** Sensible default; matches what browsers offer.
- **REMB + NACK enabled.** Without REMB the publisher can flood; without NACK packet loss looks like a stuck frame. Both are standard.
- **No transcoding.** SFU forwards raw RTP. CPU stays low and quality stays original.
- **Per-room actor model.** `RoomSession` runs on a single tokio task; all mutations are sequenced through it. Eliminates lock contention on the track registry.
- **Publisher disconnect → fan-out drop.** When a `PeerSession` ends, its tracks are removed from the registry; subscribers receive a renegotiation that drops the track.
- **Subscriber slow consumer → drop packets (don't queue).** Real-time media; queued audio/video is worse than dropped.
- **DTLS-SRTP cert is ephemeral and SFU-internal.** Browsers verify fingerprints against the SDP; there is no need for the SFU cert to chain to the HTTPS CA.

## Public interfaces

The `SfuPort` trait shape stays as defined in Phase 04 — this phase replaces the `NoopSfu` with `Sfu`.

```rust
// meet_sfu
pub struct Sfu { /* opaque */ }
impl Sfu {
    pub async fn new(cfg: SfuConfig) -> Result<Self, SfuError>;
}

#[async_trait]
impl SfuPort for Sfu { /* see Phase 04 trait */ }

pub struct SfuConfig {
    pub bind_addrs: Vec<SocketAddr>,    // UDP bind for ICE
    pub external_ip: Option<IpAddr>,    // for 1:1 NAT scenarios; None => use bind addr
    pub stats_interval: Duration,
    pub max_participants_per_room: usize,  // default 30
}
```

### Internal events (for tracing only — not stable API)

```
SfuEvent::PeerConnectionState { room, pid, state }
SfuEvent::TrackPublished      { room, pid, kind: "audio"|"video", id }
SfuEvent::TrackUnpublished    { room, pid, kind, id }
SfuEvent::IceCandidate        { room, pid, candidate: redacted }
SfuEvent::StatsSnapshot       { room, pid, bytes_in, bytes_out, packets_lost }
```

## Security considerations

- **DTLS-SRTP terminates at the SFU and is re-encrypted per subscriber.** This is the standard SFU model; documented as such in [docs/security/checklist.md](#) once Phase 09 lands.
- **Track ownership enforcement.** A subscriber cannot publish under another participant's pid; the SFU keys all bookkeeping on the authenticated `pid` from the WS connection, not on any client-supplied identifier.
- **Per-room participant cap (default 30, configurable, hard-max 50).** Protects against resource exhaustion if a token leaks. Phase 09 revisits the value.
- **Per-PC inbound bandwidth cap.** REMB pushes back when a publisher exceeds the budget; webrtc-rs default suffices, but we set a 4 Mbps ceiling per publisher.
- **ICE consent freshness.** Default in webrtc-rs (every 5s); kept on.
- **No turn:// servers in v1.** ICE candidates are host-only. If the deployment is behind NAT, the operator configures `external_ip` (1:1 mapping). Documented in `docs/INSTALL.md`.
- **No SDP munging.** The SFU answers what the client offers, modulo codec filtering. Avoids the long history of SDP-rewriting bugs.
- **Cleanup verified at the OS level.** Each `PeerSession` drop unbinds its UDP allocation; an integration test counts open UDP sockets before and after a session.
- Cross-references: prompt §4.9.

## Test plan

- **Unit (meet-sfu):**
  - `RoomSession` add-publisher / add-subscriber / unsubscribe; assertions on the registry.
  - `SfuConfig` parsing and validation.
  - Stats snapshot helpers (mock peer connection).
- **Integration (`tests/sfu_loop.rs`):**
  - Two webrtc-rs clients join the same room.
  - A publishes a synthetic audio track (silence with known RTP markers).
  - B subscribes and receives the markers within 2s.
  - A disconnects; B observes the track being removed within 1s.
  - 15 clients can join and exchange tracks without panic; no memory growth over 60s steady-state.
- **Manual / load:**
  - `scripts/loadtest_sfu.sh` runs 4 rooms × 15 dummy clients for 5 minutes; informational metrics captured to a file.
- **Brave-driven Playwright** (deferred to Phase 07): real `getUserMedia` from two browsers with fake devices, video pixel checksum on the receiver.

## Acceptance criteria

- [ ] `Sfu::new` starts and `SfuPort` is wired in `meet-server`.
- [ ] Two synthetic peers can exchange audio + video through the SFU end-to-end.
- [ ] Track removal on publisher disconnect is observed by subscribers within 1s.
- [ ] 15-participant room runs steady-state for 60s without panic and without memory growth (measured in the integration test).
- [ ] Per-room cap enforced; 16th joiner receives an `Error 4413`-style message from Phase 04 ("room full") — add a new close code `4453 Room Full` to the Phase 04 table in the same PR.
- [ ] Tracing emits `SfuEvent` records for state changes; logs never include SDP bodies at info.
- [ ] `just check` is green; `just test` runs the SFU integration test.

## Open questions

- Simulcast — defer until two real users complain about quality under load. Rough plan: VP8 with `simulcast: true`, three layers (180p/360p/720p), SFU picks per subscriber based on inbound REMB.
- Recording — explicitly out of scope. If asked for, it goes through a separate "egress" component; the SFU stays passive.
- Audio level extension (RFC 6464) — recommend enabling; useful for active-speaker detection in Phase 07. Decision: enable.
- IPv6 — supported by webrtc-rs; verify the operator can list IPv6 in `bind_addrs`.

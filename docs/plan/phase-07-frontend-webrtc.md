# Phase 07 — Frontend WebRTC

## Goal

Wire real WebRTC into the room shell. The `/r/:id` page negotiates a single `RTCPeerConnection` against the SFU using the signaling channel from Phase 04, captures local audio + video via `getUserMedia`, renders a responsive video grid (1–20 tiles), surfaces per-peer connection quality and active-speaker hints, and provides controls (mute mic, camera off, leave). Includes reconnect with exponential backoff and clear permission-denial flows.

## Deliverables

- `frontend/src/rtc/signaling.ts` — WebSocket client wrapping `wss://<host>/ws`; sends `Join` with token; emits typed events for every `ServerMsg` variant.
- `frontend/src/rtc/pc.ts` — `PeerConnectionManager`: builds the `RTCPeerConnection`, handles offer/answer/ICE with the SFU, attaches local tracks, maps remote tracks to peer ids, exposes a stable event stream.
- `frontend/src/rtc/media.ts` — `getUserMedia` wrapper with constraints presets (`audio-only`, `audio+camera`), permission-denial handling, device enumeration.
- `frontend/src/rtc/reconnect.ts` — exponential backoff scheduler (1s, 2s, 4s, 8s, 16s, cap 30s; reset on success).
- `frontend/src/rtc/active_speaker.ts` — observe `getStats` audio-level samples; emit "the loudest peer right now".
- `frontend/src/rtc/quality.ts` — observe RTCP stats per peer; output a discrete level (`good` | `ok` | `bad`).
- `frontend/src/pages/Room.tsx` — full implementation: header, video grid, controls bar, chat panel placeholder (Phase 08 fills it).
- `frontend/src/components/room/VideoTile.tsx` — single tile: video element, name label, mic-muted indicator, quality dot, "speaking" ring.
- `frontend/src/components/room/Grid.tsx` — auto layout for 1, 2, 3, 4, 6, 9, 12, 16, 20 tiles (clamped responsive `grid-template-columns`).
- `frontend/src/components/room/Controls.tsx` — mute, camera, leave, settings (device picker).
- `frontend/src/components/room/PermissionWizard.tsx` — pre-roll modal that requests permissions and explains denial recovery.
- Playwright E2E with two Brave instances proving audio + video flow.

## Design decisions

- **Single PC per page, not per peer.** Matches Phase 05 SFU model. Adding a peer triggers an SFU renegotiation, not a new PC.
- **Lazy media access.** `getUserMedia` runs only after the user clicks "join" in the `PermissionWizard`. Avoids a permission prompt the moment someone lands on `/r/:id`.
- **Video element `playsInline + autoplay + muted (for self-view)`.** Required to avoid mobile Safari autoplay refusals (T-meet supports the broader browser set even if v1 ships on Brave).
- **Self-view is local-only.** We don't loop our own outbound track; we render the local `MediaStream`.
- **Active speaker via audio level RFC 6464 stats.** Smooth with a 500ms window; switches only when a new peer is consistently louder for two windows.
- **Quality dial is RTCP-driven.** Packet loss > 5% → `bad`; jitter > 50ms → `ok`; otherwise `good`.
- **Reconnect logic at the signaling layer, not the PC layer.** A dropped WS triggers a new handshake but reuses the existing PC if its state is `connected`; if the PC has died, we drop and rebuild.
- **Controls bar lives at the bottom on desktop, floats over the grid.** Standard convention; reduces vertical scroll.
- **Settings modal lists devices via `navigator.mediaDevices.enumerateDevices()`** after first permission grant. Without permission the modal explains that device names are hidden by the browser.
- **Grid auto-layout.** Pre-computed templates per tile count; falls back to a flex wrap for >20 tiles with smaller tiles.

## Public interfaces

```ts
// rtc/signaling.ts
export type ServerEvent =
  | { type: "Joined", you: Peer, peers: Peer[], room: { id: string; name: string } }
  | { type: "PeerJoined", peer: Peer }
  | { type: "PeerLeft", pid: string }
  | { type: "Offer", sdp: string, from: "sfu" }
  | { type: "Answer", sdp: string, from: "sfu" }
  | { type: "IceCandidate", candidate: RTCIceCandidateInit, from: "sfu" }
  | { type: "Chat", ciphertext: string, nonce: string, from: string }
  | { type: "Pong", ts_client: number, ts_server: number }
  | { type: "Error", code: number, message: string };

export interface SignalingClient {
  connect(url: string, token: string): Promise<void>;
  send(msg: ClientMsg): void;
  on<E extends ServerEvent["type"]>(type: E, h: (ev: Extract<ServerEvent, { type: E }>) => void): () => void;
  close(): void;
  readonly state: "idle"|"connecting"|"open"|"closing"|"closed";
}

// rtc/pc.ts
export interface PeerConnectionManager {
  attachLocalTracks(stream: MediaStream): Promise<void>;
  onRemoteTrack(h: (pid: string, track: MediaStreamTrack) => void): () => void;
  onConnectionStateChange(h: (s: RTCPeerConnectionState) => void): () => void;
  close(): void;
}
```

## Security considerations

- **No camera/mic until consent.** The wizard makes the consent explicit; no surprise prompts.
- **Token never reaches the browser as a query-string fragment.** It travels from Zustand → first WS message only.
- **`MediaStream`s are released on leave.** Tracks `stop()`'d to drop the hardware indicators promptly.
- **Active-speaker timing data stays client-side.** It would otherwise leak conversational patterns to the server.
- **Quality stats are local-only.** Same reason.
- **Display names from peers are React-rendered (escaped) and length-capped on render (`truncate` Tailwind class).** Prevents layout abuse with very long names.
- **DTLS-SRTP fingerprint check is the browser's responsibility;** we never disable it.
- **Trickle ICE only — no preflight gathering.** Reduces join latency and limits the network surface during setup.
- Cross-references: prompt §4.9, §4.10, §4.14.

## Test plan

- **Unit (Vitest):**
  - `reconnect.ts` backoff schedule matches the documented sequence.
  - `active_speaker.ts` selects the loudest peer over a two-window confirmation.
  - `quality.ts` thresholds map correctly to `good`/`ok`/`bad`.
- **Component:**
  - `Grid` snapshot tests at 1, 4, 9, 12, 20 tiles.
  - `VideoTile` shows muted-mic indicator when the prop is set.
  - `Controls` calls the right handlers on click.
- **E2E (Playwright + Brave):**
  - Two Brave instances with `--use-fake-device-for-media-stream` join the same room with a known fake video; receiver pixel checksum matches sender's pattern.
  - Mute on Brave A; Brave B's UI shows the muted indicator within 1s.
  - Kill the WS on Brave A; signaling reconnects per the backoff schedule.
- **Manual:**
  - Real laptops on the same LAN, no echo, latency feels normal (<200ms one-way perceived).
  - Permission denied → wizard explains how to recover; refreshing the page lets the user retry.

## Acceptance criteria

- [ ] Two Brave clients exchange real audio + video through the SFU.
- [ ] Mute / unmute and camera off / on propagate within 1s to the other client.
- [ ] Active-speaker indicator switches correctly under controlled audio.
- [ ] Connection-quality dial reflects degraded conditions (verified by injecting `tc qdisc` packet loss in a manual test).
- [ ] WS drop triggers reconnect with the documented backoff; PC stays up if it was healthy.
- [ ] Grid layouts render at 1, 4, 9, 12, 20 tiles without overlap.
- [ ] Permission-denial flow shows the wizard with recovery instructions.
- [ ] No tokens, no ciphertext, no IPs in browser console at any point.
- [ ] `just check` is green; Playwright E2E passes.

## Open questions

- Whether to add a "raise hand" affordance. Recommendation: not in v1 — single-purpose room.
- Bandwidth caps per user — defer; rely on REMB. If a user requests a hard ceiling, expose it via the settings modal in a follow-up.
- Whether to allow background blur. Recommendation: defer; would need a `wasm-pack`'d model and a chunk size bump.
- Picture-in-picture for the active speaker. Recommendation: defer; nice-to-have.

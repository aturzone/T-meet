# Phase 08 — E2E Chat

## Goal

Add an end-to-end encrypted chat panel to the room. Each participant generates an ephemeral X25519 keypair on join and shares the public key via the signaling channel (`Joined.you.pubkey` and `PeerJoined.peer.pubkey`). Outgoing chat messages are sealed per recipient using `libsodium`'s sealed-box construction; the server fans out ciphertext only. The chat panel renders sender, timestamp, and decrypted plaintext; scrollback is in-memory only and not persisted anywhere.

## Deliverables

- `frontend/src/chat/keys.ts` — generate the per-session keypair via `libsodium-wrappers-sumo`; expose `myPublic`, `myPrivate` (held in module-scope memory only).
- `frontend/src/chat/seal.ts` — `sealForRecipient(recipientPubkey, plaintext) -> { ciphertext, nonce }` using `crypto_box_seal`; `open(senderCiphertext) -> plaintext | null`.
- `frontend/src/chat/store.ts` — Zustand slice holding the per-peer keys (`Map<pid, Uint8Array>`) and the message log (`{ id, fromPid, fromName, ts, text }[]`).
- `frontend/src/chat/wire.ts` — `sendChat(text, targetPid?: string)` constructs one signaling `Chat` message per recipient (or for all known peers); on receive, decrypt and push to the store.
- `frontend/src/components/room/ChatPanel.tsx` — collapsible side panel; input + send; rendered message list with sender name, time, and text (no markdown — plaintext only in v1).
- `frontend/src/components/room/ChatMessage.tsx` — single row.
- Update `rtc/signaling.ts` (Phase 07) to expose `Joined.you.pubkey` and to populate `PeerJoined.peer.pubkey`; update `Phase 04` interface to make `pubkey` REQUIRED once Phase 08 lands.
- Update `Room.tsx` to wire chat into the page chrome.
- Tests: unit tests for seal/open; component tests for the panel; Playwright E2E proving the server never sees plaintext.

## Design decisions

- **Sealed-box per recipient over shared room key.** For 10–20 participants, the constant-factor overhead of N sealed boxes per message is invisible compared to the network. Shared room keys would require rekeying on every join and leave, which is correctness-fragile. Sealed boxes are stateless and ship one well-understood primitive.
- **Ephemeral X25519 per session.** No long-term identity; if a key is compromised, only the live session is affected, and the room ends with no recoverable artifact.
- **`libsodium-wrappers-sumo` not `nacl`.** The `sumo` variant ships the full API surface; we use the modern `crypto_box_seal` flow which is itself an ephemeral-key construction on top of X25519 + XSalsa20 + Poly1305.
- **WS-based chat, not data channel.** Easier to reason about ordering and presence with a single transport. A future Phase 09+ change could move chat to a data channel for one-fewer-server-roundtrip; not blocking for v1.
- **No persistence.** Scrollback dies with the page. Documented in the UI ("messages disappear when you leave") and in `docs/CA-TRUST.md`.
- **No markdown.** Markdown opens an XSS surface and adds parser complexity; defer indefinitely.
- **Reject messages from unknown pids.** A `Chat` message from a `from` we don't have a pubkey for is dropped silently and logged at debug.
- **Server-side: `Chat.recipients` is a single `to` field — `"all"` or a specific pid.** When the client wants per-recipient sealed boxes, it sends N messages with explicit `to: <pid>` for each peer. Keeps the server simple.

## Public interfaces

### Signaling (refinement of Phase 04 `Chat`)

```jsonc
// client -> server
{ "v": 1, "type": "Chat",
  "ciphertext": "<base64>",                 // sealed-box ciphertext for ONE recipient
  "nonce": "<base64-24>",                   // included for future-proofing; not used by sealed-box but reserved
  "to": "<pid>" | "all"                     // "all" => server fans out without per-recipient sealing
                                            // (used only for v1 *unsealed* metadata if ever needed; chat always uses pid)
}

// server -> client (unchanged)
{ "v": 1, "type": "Chat",
  "ciphertext": "<base64>",
  "nonce": "<base64-24>",
  "from": "<pid>" }
```

In v1, the client always sends N copies, one per recipient pid. The `"all"` form is reserved.

### Joined / PeerJoined

`pubkey` was nullable in Phase 04. In Phase 08 it becomes required — old clients (none exist yet) would simply not receive the upgrade.

### TS module surfaces

```ts
// chat/keys.ts
export interface SessionKeys { publicKey: Uint8Array; privateKey: Uint8Array; }
export async function initSessionKeys(): Promise<SessionKeys>;

// chat/seal.ts
export function sealForRecipient(recipient: Uint8Array, plaintext: string): { ciphertext: Uint8Array };
export function openIncoming(myKeys: SessionKeys, ciphertext: Uint8Array): string | null;

// chat/store.ts
export interface ChatMsg { id: string; fromPid: string; fromName: string; ts: number; text: string; }
export interface ChatState {
  myKeys?: SessionKeys;
  peerKeys: Map<string, Uint8Array>;       // pid -> pubkey
  messages: ChatMsg[];
  appendOutgoing(text: string): void;       // sends + appends locally
  appendIncoming(msg: ChatMsg): void;
}
```

## Security considerations

- **Sealed boxes provide confidentiality with ephemeral-key forward secrecy on the sender side.** Recipient compromise reveals only messages a peer chose to send to that recipient — not unrelated traffic.
- **No authentication of sender identity beyond the WS-level `from: <pid>`.** The signaling layer guarantees that `<pid>` is the authenticated peer; if that guarantee breaks, chat trust breaks too. Documented in `docs/security/chat-model.md` (created in Phase 09).
- **No message ordering guarantee across peers.** Local store stamps `ts` from `performance.now() + Date.now()` baseline; reorder visually by ts at render time.
- **No message authenticity proofs.** A malicious server could re-route a ciphertext from A's box-for-B to A's box-for-C — but it would decrypt to garbage because the sealed-box opens only with C's private key. The risk surface is the server *withholding* messages, which it can always do anyway.
- **Plaintext lives only in the recipient's RAM.** Never logged; never written to storage; never serialized.
- **Key material zeroized on `leave`.** `myPrivate` overwritten to zeros; `peerKeys` map cleared.
- **Length-cap on plaintext: 2 KiB.** Longer messages should use a paste link in a different system; we're not building a wiki.
- **Rate limit chat fan-out at the server: max 20 outbound chat messages per pid per minute.** Phase 09 hardens this; tracked in `Open questions`.
- **`Chat.ciphertext` never logged at info; debug only counts fan-out fan-in.**
- Cross-references: prompt §4.8, §4.10.

## Test plan

- **Unit (Vitest):**
  - `initSessionKeys` returns a 32-byte public key and a 32-byte private key.
  - `sealForRecipient` then `openIncoming` round-trips for a matched keypair.
  - `openIncoming` returns `null` for a tampered ciphertext.
  - Store appends incoming and outgoing in order.
- **Component:**
  - `ChatPanel` sends a message that triggers `wire.sendChat` once per known peer.
  - Long-press / hover on a message shows the timestamp (or whatever interaction is decided in Phase 06).
- **E2E (Playwright + Brave):**
  - Two clients in a room.
  - Client A sends "hello"; client B sees "hello".
  - A network capture inside the server (sqlite audit log + tracing) shows the ciphertext byte length but never the plaintext.
- **Manual:**
  - Three-way room, A sends a message to B and C; both receive it; A also sees their own outgoing local copy.

## Acceptance criteria

- [x] Each participant generates an X25519 keypair on join. After `Joined`, the client sends `ClientMsg::Announce { pubkey }` and the server fans `ServerMsg::PeerUpdated { peer: { …, pubkey } }` to every participant so the table stays consistent. New joiners also learn existing peers' pubkeys via `PeerUpdated` (re-broadcast triggers when their own announce arrives).
- [x] Outgoing chat sends N sealed boxes (one per recipient); the server fan-out in `routes_public::chat` was already in place from Phase 04 — the wire is unchanged.
- [x] Receivers decrypt only with their private key (`crypto_box_seal_open` requires the recipient's keypair); the server only sees ciphertext.
- [x] Audit log records `chat.fanout` counts but never plaintext or ciphertext bodies. (Phase 04's chat handler logs `chat.fanout` at debug only with a count; no body fields.)
- [x] Private key is zeroized on leave via `sodium.memzero(privateKey)` in `clearSessionKeys`; the cached reference is dropped.
- [x] Plaintext length-cap (2 KiB) enforced — `sealForRecipient` throws above `MAX_PLAINTEXT_BYTES`; the chat input also caps `maxLength` so oversize text is impossible at the keyboard layer.
- [x] Display names in chat are React-escaped — `ChatMessage` renders `{msg.fromName}` and `{msg.text}` as text nodes, no `dangerouslySetInnerHTML` anywhere in the chat path.
- [x] `just check` is green. ~~Playwright proves plaintext absence in server logs~~ — **deferred to Phase 09's hardening sweep** where the Brave + fake-device harness lands. The server-side audit log unit test in Phase 04 (`access` logs at info show no chat fields) covers the "no plaintext in server logs" invariant from the static-analysis angle.

## Open questions

- Whether to add typing indicators. Recommendation: defer — typing leaks rhythm and adds a server-driven channel for no functional gain.
- Whether to add file transfer (data channel). Recommendation: not in v1 — out of scope per the prompt.
- Per-peer "ignore" / mute in chat. Recommendation: yes, as a client-side filter only; trivial in Phase 09.
- Server-side chat rate limit value — 20/min/pid is a guess; revisit when the first live use shows usage patterns.

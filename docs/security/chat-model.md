# Chat E2E trust model

T-meet chat is end-to-end encrypted between participants. The server forwards
ciphertext + recipient hints; it cannot read plaintext.

## Primitive

`crypto_box_seal` from libsodium (X25519 + XSalsa20-Poly1305).

Each participant generates an X25519 keypair on `Joined`, holds it in
module-scope memory, and announces the public key via
`ClientMsg::Announce { pubkey }`. The server records the pubkey in the
participant's `PeerDescriptor` and re-broadcasts `ServerMsg::PeerUpdated` so
every peer learns it.

## Sender flow

1. Read the recipient list (`useSession.peers`).
2. For each recipient with a known pubkey, call
   `sealForRecipient(recipient.pubkey, plaintext)`.
3. Send one `ClientMsg::Chat { ciphertext, nonce: "", to: <pid> }` per
   recipient. The `nonce` field is reserved for a future flavor of chat;
   sealed boxes don't need it (the ephemeral sender key is in the
   ciphertext header).
4. Append the message locally so the sender sees their own copy.

## Recipient flow

1. Receive `ServerMsg::Chat { ciphertext, from }`.
2. Call `openIncoming(ciphertext, mySessionKeys)`.
3. If `null`, drop silently — wrong recipient or tampered ciphertext.
4. Otherwise, append to the chat store with `from` (mapped to the
   peer's display name).

## What the server can do

- See ciphertext byte length.
- See `from` and `to` pids.
- Reorder messages (delivery is best-effort).
- Withhold messages from specific recipients.

## What the server can't do

- Decrypt the plaintext.
- Substitute a ciphertext from A's box-for-B into A's box-for-C — sealed
  boxes are bound to the recipient pubkey at seal time, so a swap
  decrypts to garbage (or — more commonly — fails the Poly1305 check and
  is dropped silently).
- Forge a message from A — sealed boxes don't authenticate the sender at
  the crypto layer; sender attribution is by the WS-authenticated `from`
  pid that the server stamps. If the server lies about `from`, the
  message still decrypts but is mis-attributed. The threat there is
  malicious-server attribution, which is already in the trust model.

## Forward secrecy

- Keypairs are ephemeral per session. Closing the page (or pressing
  Leave) calls `clearSessionKeys` which `sodium.memzero`'s the private
  key. Captured ciphertext is undecryptable once the session ends.
- The sealed-box construction itself uses a fresh ephemeral sender key
  per message, so leaking a *sender's* long-term key (if there were one)
  wouldn't help decrypt past traffic. T-meet doesn't have long-term keys
  anyway.

## What we don't do (intentional)

- No message persistence. Scrollback dies with the page.
- No file transfer.
- No typing indicators.
- No read receipts.
- No long-term identity. Each session is a fresh keypair.

## Length cap

Plaintext is capped at 2 KiB (`MAX_PLAINTEXT_BYTES`). The chat input also
caps `maxLength` at the same value, so oversize input is impossible to
type. Larger messages should be shared via a different channel — T-meet
doesn't want to be a wiki.

## Rate limit

Server-side: 20 chat fan-outs per pid per minute (Phase 09). Hitting the
limit returns `ServerMsg::Error { code: 4429 }` and the over-limit chat
is dropped without fan-out.

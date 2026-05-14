/** Wire glue: encrypts outgoing chat, decrypts incoming, drives the store. */

import type { SignalingClient } from "../rtc/signaling";
import type { Peer } from "../lib/store";
import {
  initSessionKeys,
  pubkeyBase64,
  pubkeyFromBase64,
} from "./keys";
import {
  ciphertextFromWire,
  ciphertextToWire,
  openIncoming,
  sealForRecipient,
} from "./seal";
import { useChat, type ChatMessage } from "./store";

const PROTO = 1;

/** Hand-shake on connect: tell the server our pubkey. */
export async function announcePubkey(
  signaling: SignalingClient,
): Promise<void> {
  const keys = await initSessionKeys();
  const pk = await pubkeyBase64(keys.publicKey);
  signaling.send({ type: "Announce", v: PROTO, pubkey: pk });
}

/** Record a peer's pubkey (from Joined / PeerJoined / PeerUpdated). */
export async function recordPeerKey(
  pid: string,
  pubkey: string,
): Promise<void> {
  const bytes = await pubkeyFromBase64(pubkey);
  useChat.getState().setPeerKey(pid, bytes);
}

/** Send `text` to every peer in `peers`, one sealed-box per recipient.
 *  Returns the number of recipients we successfully encrypted for. */
export async function sendChat(
  signaling: SignalingClient,
  text: string,
  peers: Peer[],
  selfPid: string,
  selfName: string,
): Promise<number> {
  const trimmed = text.trim();
  if (!trimmed) return 0;

  let sent = 0;
  for (const p of peers) {
    if (p.pid === selfPid) continue;
    const keys = useChat.getState().peerKeys.get(p.pid);
    if (!keys) continue;
    const ciphertext = await sealForRecipient(keys, trimmed);
    const ct = await ciphertextToWire(ciphertext);
    signaling.send({
      type: "Chat",
      v: PROTO,
      ciphertext: ct,
      nonce: "",
      to: p.pid,
    });
    sent += 1;
  }

  // Local echo: append to our own log immediately so the sender sees it.
  const msg: ChatMessage = {
    id: crypto.randomUUID(),
    fromPid: selfPid,
    fromName: selfName,
    ts: Date.now(),
    text: trimmed,
  };
  useChat.getState().append(msg);

  return sent;
}

/** Try to decrypt an incoming Chat frame; on success, append to the store. */
export async function ingestChat(args: {
  fromPid: string;
  fromName: string;
  ciphertext: string;
}): Promise<void> {
  const keys = await initSessionKeys();
  let bytes: Uint8Array;
  try {
    bytes = await ciphertextFromWire(args.ciphertext);
  } catch {
    return;
  }
  const text = await openIncoming(bytes, keys);
  if (text === null) return; // not for us / tampered
  useChat.getState().append({
    id: crypto.randomUUID(),
    fromPid: args.fromPid,
    fromName: args.fromName,
    ts: Date.now(),
    text,
  });
}

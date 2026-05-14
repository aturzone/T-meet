/** `crypto_box_seal` per-recipient encryption. */

import sodium from "libsodium-wrappers-sumo";
import type { SessionKeys } from "./keys";

/** Plaintext message cap — keep chat small so it doesn't blow the 64 KiB WS
 *  frame budget. */
export const MAX_PLAINTEXT_BYTES = 2 * 1024;

let initPromise: Promise<void> | null = null;

async function ensureReady(): Promise<void> {
  if (!initPromise) {
    initPromise = sodium.ready;
  }
  await initPromise;
}

/** Seal `plaintext` for `recipient`. Returns the ciphertext as Uint8Array. */
export async function sealForRecipient(
  recipient: Uint8Array,
  plaintext: string,
): Promise<Uint8Array> {
  await ensureReady();
  // sodium.from_string returns a Uint8Array, but in jsdom + ESM that may be
  // a different realm than libsodium expects. Force-copy through Uint8Array
  // in the *current* realm to make the instanceof check pass.
  const raw = sodium.from_string(plaintext);
  const encoded = new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength);
  if (encoded.length > MAX_PLAINTEXT_BYTES) {
    throw new Error(`message exceeds ${MAX_PLAINTEXT_BYTES} bytes`);
  }
  return sodium.crypto_box_seal(encoded, new Uint8Array(recipient));
}

/** Open a sealed-box message with our session keys. Returns the plaintext, or
 *  `null` if decryption fails (wrong recipient / tampered / etc). */
export async function openIncoming(
  ciphertext: Uint8Array,
  keys: SessionKeys,
): Promise<string | null> {
  await ensureReady();
  try {
    const plain = sodium.crypto_box_seal_open(
      ciphertext,
      keys.publicKey,
      keys.privateKey,
    );
    return sodium.to_string(plain);
  } catch {
    return null;
  }
}

/** Helpers to bridge the wire's base64 string ↔ Uint8Array. */
export async function ciphertextToWire(
  bytes: Uint8Array,
): Promise<string> {
  await ensureReady();
  return sodium.to_base64(bytes, sodium.base64_variants.URLSAFE_NO_PADDING);
}

export async function ciphertextFromWire(
  s: string,
): Promise<Uint8Array> {
  await ensureReady();
  return sodium.from_base64(s, sodium.base64_variants.URLSAFE_NO_PADDING);
}

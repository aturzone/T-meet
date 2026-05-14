/** Ephemeral X25519 keypair per session, generated via libsodium. */

import sodium from "libsodium-wrappers-sumo";

export interface SessionKeys {
  publicKey: Uint8Array;
  privateKey: Uint8Array;
}

let cached: SessionKeys | null = null;
let initPromise: Promise<void> | null = null;

async function ensureReady(): Promise<void> {
  if (!initPromise) {
    initPromise = sodium.ready;
  }
  await initPromise;
}

/** Generate (or return the cached) per-session keypair. */
export async function initSessionKeys(): Promise<SessionKeys> {
  await ensureReady();
  if (cached) return cached;
  const kp = sodium.crypto_box_keypair();
  cached = {
    publicKey: kp.publicKey,
    privateKey: kp.privateKey,
  };
  return cached;
}

/** Zero out and forget the keypair. Call on `leave`. */
export async function clearSessionKeys(): Promise<void> {
  await ensureReady();
  if (!cached) return;
  sodium.memzero(cached.privateKey);
  // The public key isn't sensitive but we drop the reference anyway.
  cached = null;
}

/** Base64 (no padding) encode a key for the signaling channel. */
export async function pubkeyBase64(key: Uint8Array): Promise<string> {
  await ensureReady();
  return sodium.to_base64(key, sodium.base64_variants.URLSAFE_NO_PADDING);
}

/** Inverse of `pubkeyBase64`. */
export async function pubkeyFromBase64(s: string): Promise<Uint8Array> {
  await ensureReady();
  return sodium.from_base64(s, sodium.base64_variants.URLSAFE_NO_PADDING);
}

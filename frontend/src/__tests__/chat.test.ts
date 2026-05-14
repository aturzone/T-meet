import { describe, it, expect, beforeEach } from "vitest";
import sodium from "libsodium-wrappers-sumo";

import { initSessionKeys, clearSessionKeys } from "../chat/keys";
import {
  ciphertextFromWire,
  ciphertextToWire,
  openIncoming,
  sealForRecipient,
  MAX_PLAINTEXT_BYTES,
} from "../chat/seal";
import { useChat } from "../chat/store";

beforeEach(async () => {
  await clearSessionKeys();
  useChat.getState().clear();
});

describe("chat keys", () => {
  it("initSessionKeys returns 32-byte keys and is idempotent", async () => {
    const a = await initSessionKeys();
    expect(a.publicKey.length).toBe(32);
    expect(a.privateKey.length).toBe(32);
    const b = await initSessionKeys();
    expect(a).toBe(b); // cached
  });

  it("clearSessionKeys zeroes the private key", async () => {
    const k = await initSessionKeys();
    const before = new Uint8Array(k.privateKey);
    await clearSessionKeys();
    // The cached reference is wiped; verify the old bytes were zeroed.
    expect(before.some((b) => b !== 0)).toBe(true); // had non-zero before
    expect(k.privateKey.every((b) => b === 0)).toBe(true);
  });
});

describe("seal / open", () => {
  it("round-trips with a matching recipient keypair", async () => {
    await sodium.ready;
    const recipient = sodium.crypto_box_keypair();
    // Simulate "I am the recipient" by replacing the cached keys.
    // Easiest: bypass the cache by sealing for recipient.publicKey and
    // opening with the recipient's own keys.
    const ciphertext = await sealForRecipient(
      recipient.publicKey,
      "hello world",
    );
    const wire = await ciphertextToWire(ciphertext);
    const bytes = await ciphertextFromWire(wire);
    const text = await openIncoming(bytes, {
      publicKey: recipient.publicKey,
      privateKey: recipient.privateKey,
    });
    expect(text).toBe("hello world");
  });

  it("returns null on tampered ciphertext", async () => {
    await sodium.ready;
    const recipient = sodium.crypto_box_keypair();
    const ciphertext = await sealForRecipient(recipient.publicKey, "secret");
    const last = ciphertext.length - 1;
    const old = ciphertext[last] ?? 0;
    ciphertext[last] = old ^ 1;
    const text = await openIncoming(ciphertext, {
      publicKey: recipient.publicKey,
      privateKey: recipient.privateKey,
    });
    expect(text).toBeNull();
  });

  it("returns null when opened with the wrong key", async () => {
    await sodium.ready;
    const intended = sodium.crypto_box_keypair();
    const eve = sodium.crypto_box_keypair();
    const ciphertext = await sealForRecipient(intended.publicKey, "x");
    const text = await openIncoming(ciphertext, {
      publicKey: eve.publicKey,
      privateKey: eve.privateKey,
    });
    expect(text).toBeNull();
  });

  it("rejects oversize plaintext", async () => {
    await sodium.ready;
    const r = sodium.crypto_box_keypair();
    const big = "a".repeat(MAX_PLAINTEXT_BYTES + 1);
    await expect(sealForRecipient(r.publicKey, big)).rejects.toThrow();
  });
});

describe("chat store", () => {
  it("setPeerKey and removePeer behave", () => {
    const k = new Uint8Array([1, 2, 3]);
    useChat.getState().setPeerKey("p1", k);
    expect(useChat.getState().peerKeys.get("p1")).toEqual(k);
    useChat.getState().removePeer("p1");
    expect(useChat.getState().peerKeys.has("p1")).toBe(false);
  });

  it("append preserves order", () => {
    const s = useChat.getState();
    s.append({ id: "a", fromPid: "x", fromName: "X", ts: 1, text: "hi" });
    s.append({ id: "b", fromPid: "y", fromName: "Y", ts: 2, text: "yo" });
    expect(useChat.getState().messages.map((m) => m.id)).toEqual(["a", "b"]);
  });
});

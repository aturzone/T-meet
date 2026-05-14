/** Chat-specific Zustand slice: per-peer keys + the message log. */

import { create } from "zustand";

export interface ChatMessage {
  id: string;
  fromPid: string;
  fromName: string;
  ts: number;
  text: string;
}

interface ChatState {
  /** pid → 32-byte X25519 public key */
  peerKeys: Map<string, Uint8Array>;
  messages: ChatMessage[];
  setPeerKey: (pid: string, pubkey: Uint8Array) => void;
  removePeer: (pid: string) => void;
  append: (msg: ChatMessage) => void;
  clear: () => void;
}

export const useChat = create<ChatState>((set) => ({
  peerKeys: new Map(),
  messages: [],
  setPeerKey: (pid, pubkey) =>
    set((state) => {
      const next = new Map(state.peerKeys);
      next.set(pid, pubkey);
      return { peerKeys: next };
    }),
  removePeer: (pid) =>
    set((state) => {
      if (!state.peerKeys.has(pid)) return state;
      const next = new Map(state.peerKeys);
      next.delete(pid);
      return { peerKeys: next };
    }),
  append: (msg) => set((state) => ({ messages: [...state.messages, msg] })),
  clear: () => set({ peerKeys: new Map(), messages: [] }),
}));

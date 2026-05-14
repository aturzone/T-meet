import { create } from "zustand";
import type { JoinResponse } from "./schemas";

export interface Peer {
  pid: string;
  displayName: string;
  pubkey?: string;
}

export interface SessionState {
  roomId?: string | undefined;
  joinToken?: string | undefined;
  participantId?: string | undefined;
  displayName?: string | undefined;
  wsUrl?: string | undefined;
  peers: Peer[];
  setSession: (s: JoinResponse & { roomId: string; displayName: string }) => void;
  setPeers: (peers: Peer[]) => void;
  addPeer: (p: Peer) => void;
  removePeer: (pid: string) => void;
  clear: () => void;
}

export const useSession = create<SessionState>((set) => ({
  peers: [],
  setSession: (s) =>
    set({
      roomId: s.roomId,
      joinToken: s.join_token,
      participantId: s.participant_id,
      displayName: s.displayName,
      wsUrl: s.ws_url,
      peers: [],
    }),
  setPeers: (peers) => set({ peers }),
  addPeer: (p) =>
    set((state) =>
      state.peers.some((x) => x.pid === p.pid)
        ? state
        : { peers: [...state.peers, p] },
    ),
  removePeer: (pid) =>
    set((state) => ({ peers: state.peers.filter((p) => p.pid !== pid) })),
  clear: () =>
    set({
      roomId: undefined,
      joinToken: undefined,
      participantId: undefined,
      displayName: undefined,
      wsUrl: undefined,
      peers: [],
    }),
}));

export type ToastLevel = "info" | "warn" | "error";

export interface Toast {
  id: string;
  level: ToastLevel;
  message: string;
}

export interface UiState {
  toasts: Toast[];
  pushToast: (level: ToastLevel, message: string) => void;
  dismissToast: (id: string) => void;
}

export const useUi = create<UiState>((set) => ({
  toasts: [],
  pushToast: (level, message) =>
    set((state) => ({
      toasts: [
        ...state.toasts,
        { id: crypto.randomUUID(), level, message },
      ],
    })),
  dismissToast: (id) =>
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
}));

/** Wire types mirrored from `meet_core::signaling`. */

export interface PeerDescriptor {
  pid: string;
  display_name: string;
  pubkey?: string;
}

export interface RoomDescriptor {
  id: string;
  name: string;
}

export type ServerEvent =
  | {
      type: "Joined";
      v: number;
      you: PeerDescriptor;
      peers: PeerDescriptor[];
      room: RoomDescriptor;
    }
  | { type: "PeerJoined"; v: number; peer: PeerDescriptor }
  | { type: "PeerLeft"; v: number; pid: string }
  | { type: "Offer"; v: number; sdp: string; from: string }
  | { type: "Answer"; v: number; sdp: string; from: string }
  | { type: "IceCandidate"; v: number; candidate: unknown; from: string }
  | {
      type: "Chat";
      v: number;
      ciphertext: string;
      nonce: string;
      from: string;
    }
  | { type: "Pong"; v: number; ts_client: number; ts_server: number }
  | { type: "Error"; v: number; code: number; message: string };

export type ClientMsg =
  | { type: "Join"; v: number; token: string }
  | { type: "Offer"; v: number; sdp: string; to: string }
  | { type: "Answer"; v: number; sdp: string; to: string }
  | { type: "IceCandidate"; v: number; candidate: unknown; to: string }
  | {
      type: "Chat";
      v: number;
      ciphertext: string;
      nonce: string;
      to: string;
    }
  | { type: "Ping"; v: number; ts: number };

export const PROTOCOL_VERSION = 1;

/** Per-tab RTCPeerConnection wrapper. Single PC negotiated against the SFU.
 *
 *  Phase 07 contract:
 *  - Client offers with sendrecv audio + video transceivers (publish + receive
 *    placeholder in the same m-line).
 *  - SFU answers; client applies.
 *  - SFU's renegotiations (server-initiated offers) are received via
 *    signaling and applied here.
 *  - When the SFU forwards a track from another peer, `ontrack` fires and the
 *    consumer attaches it to the right `<video>` tile via the peer id carried
 *    on the track's `MediaStream.id` (set by the SFU).
 */

import type { SignalingClient } from "./signaling";

export type RtcEvent =
  | { type: "track"; pid: string; track: MediaStreamTrack; stream: MediaStream }
  | { type: "connection-state"; state: RTCPeerConnectionState };

type RtcListener = (ev: RtcEvent) => void;

export class PeerConnectionManager {
  private pc: RTCPeerConnection;
  private listeners = new Set<RtcListener>();
  private detachSignaling: Array<() => void> = [];

  constructor(private readonly signaling: SignalingClient) {
    this.pc = new RTCPeerConnection({
      iceServers: [], // host-only ICE — LAN deployments + 1:1 NAT
    });

    this.pc.addEventListener("track", (ev) => {
      const stream = ev.streams[0] ?? new MediaStream([ev.track]);
      // Convention: the SFU labels each forwarded stream by source pid.
      const pid = stream.id;
      this.emit({ type: "track", pid, track: ev.track, stream });
    });

    this.pc.addEventListener("connectionstatechange", () => {
      this.emit({ type: "connection-state", state: this.pc.connectionState });
    });

    this.pc.addEventListener("icecandidate", (ev) => {
      if (ev.candidate) {
        this.signaling.send({
          type: "IceCandidate",
          v: 1,
          candidate: ev.candidate.toJSON(),
          to: "sfu",
        });
      }
    });

    // SFU → client SDP / ICE.
    this.detachSignaling.push(
      this.signaling.on("Offer", async (ev) => {
        try {
          await this.pc.setRemoteDescription({ type: "offer", sdp: ev.sdp });
          const answer = await this.pc.createAnswer();
          await this.pc.setLocalDescription(answer);
          this.signaling.send({
            type: "Answer",
            v: 1,
            sdp: answer.sdp ?? "",
            to: "sfu",
          });
        } catch (e) {
          console.error("renegotiation failed", e);
        }
      }),
      this.signaling.on("Answer", async (ev) => {
        try {
          await this.pc.setRemoteDescription({ type: "answer", sdp: ev.sdp });
        } catch (e) {
          console.error("setRemoteDescription(answer) failed", e);
        }
      }),
      this.signaling.on("IceCandidate", async (ev) => {
        try {
          await this.pc.addIceCandidate(
            ev.candidate as RTCIceCandidateInit | undefined,
          );
        } catch (e) {
          console.error("addIceCandidate failed", e);
        }
      }),
    );
  }

  on(listener: RtcListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Attach the local capture and trigger an initial offer to the SFU. */
  async attachLocalAndOffer(stream: MediaStream): Promise<void> {
    for (const t of stream.getTracks()) {
      this.pc.addTrack(t, stream);
    }
    const offer = await this.pc.createOffer();
    await this.pc.setLocalDescription(offer);
    this.signaling.send({
      type: "Offer",
      v: 1,
      sdp: offer.sdp ?? "",
      to: "sfu",
    });
  }

  setMicEnabled(stream: MediaStream | null, enabled: boolean): void {
    if (!stream) return;
    for (const t of stream.getAudioTracks()) {
      t.enabled = enabled;
    }
  }

  setCameraEnabled(stream: MediaStream | null, enabled: boolean): void {
    if (!stream) return;
    for (const t of stream.getVideoTracks()) {
      t.enabled = enabled;
    }
  }

  close(): void {
    for (const detach of this.detachSignaling) detach();
    this.detachSignaling = [];
    this.pc.close();
  }

  private emit(ev: RtcEvent): void {
    for (const l of this.listeners) {
      try {
        l(ev);
      } catch (e) {
        console.error("rtc listener threw", e);
      }
    }
  }
}

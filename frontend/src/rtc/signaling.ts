/** WebSocket signaling client. Reconnects with exponential backoff and
 *  surfaces a typed event stream. */

import { Backoff } from "./reconnect";
import { PROTOCOL_VERSION, type ClientMsg, type ServerEvent } from "./types";

export type SignalingState =
  | "idle"
  | "connecting"
  | "open"
  | "closing"
  | "closed";

type Listener<E extends ServerEvent["type"]> = (
  ev: Extract<ServerEvent, { type: E }>,
) => void;

export interface SignalingOptions {
  url: string;
  token: string;
  onStateChange?: (state: SignalingState) => void;
}

export class SignalingClient {
  private socket: WebSocket | null = null;
  private state: SignalingState = "idle";
  private readonly listeners = new Map<
    ServerEvent["type"],
    Set<(ev: ServerEvent) => void>
  >();
  private readonly backoff = new Backoff();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionalClose = false;

  constructor(private readonly opts: SignalingOptions) {}

  connect(): void {
    this.intentionalClose = false;
    this.openSocket();
  }

  send(msg: ClientMsg): void {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(msg));
    }
  }

  on<E extends ServerEvent["type"]>(
    type: E,
    listener: Listener<E>,
  ): () => void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener as (ev: ServerEvent) => void);
    return () => {
      set?.delete(listener as (ev: ServerEvent) => void);
    };
  }

  close(): void {
    this.intentionalClose = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.setState("closing");
    this.socket?.close(1000, "client closing");
    this.socket = null;
    this.setState("closed");
  }

  get currentState(): SignalingState {
    return this.state;
  }

  private openSocket(): void {
    this.setState("connecting");
    let ws: WebSocket;
    try {
      ws = new WebSocket(this.opts.url);
    } catch (e) {
      console.error("ws ctor failed", e);
      this.scheduleReconnect();
      return;
    }
    this.socket = ws;

    ws.addEventListener("open", () => {
      this.setState("open");
      this.backoff.reset();
      // First message: Join with our token.
      const join: ClientMsg = {
        type: "Join",
        v: PROTOCOL_VERSION,
        token: this.opts.token,
      };
      ws.send(JSON.stringify(join));
    });

    ws.addEventListener("message", (ev: MessageEvent) => {
      if (typeof ev.data !== "string") return;
      let parsed: ServerEvent;
      try {
        parsed = JSON.parse(ev.data) as ServerEvent;
      } catch {
        return;
      }
      const set = this.listeners.get(parsed.type);
      if (!set) return;
      for (const l of set) {
        try {
          l(parsed);
        } catch (e) {
          console.error("signaling listener threw", e);
        }
      }
    });

    ws.addEventListener("close", (ev: CloseEvent) => {
      const wasOpen = this.state === "open";
      this.setState("closed");
      this.socket = null;

      // Don't reconnect on auth/protocol/idle terminal codes.
      const TERMINAL = new Set([4400, 4401, 4408, 4409, 4413, 4429, 4453]);
      if (this.intentionalClose || TERMINAL.has(ev.code)) {
        return;
      }
      // Server transient disconnect → reconnect (only meaningful if we had
      // been open at least once).
      if (wasOpen) {
        this.scheduleReconnect();
      } else {
        this.scheduleReconnect();
      }
    });

    ws.addEventListener("error", () => {
      // Browsers don't expose error detail; the `close` event follows.
    });
  }

  private scheduleReconnect(): void {
    if (this.intentionalClose) return;
    const delay = this.backoff.next();
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, delay);
  }

  private setState(s: SignalingState): void {
    this.state = s;
    this.opts.onStateChange?.(s);
  }
}

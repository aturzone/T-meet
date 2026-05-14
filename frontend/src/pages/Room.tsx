import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";

import { ChatPanel } from "../components/room/ChatPanel";
import { Controls } from "../components/room/Controls";
import { Grid } from "../components/room/Grid";
import { PermissionWizard } from "../components/room/PermissionWizard";
import { VideoTile } from "../components/room/VideoTile";
import { PeerConnectionManager } from "../rtc/pc";
import { SignalingClient, type SignalingState } from "../rtc/signaling";
import { requestMedia, stopTracks, type MediaError } from "../rtc/media";
import {
  announcePubkey,
  ingestChat,
  recordPeerKey,
} from "../chat/wire";
import { clearSessionKeys } from "../chat/keys";
import { useChat } from "../chat/store";
import { useSession, useUi } from "../lib/store";

interface RemoteFeed {
  pid: string;
  stream: MediaStream;
}

export default function Room() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const session = useSession();
  const pushToast = useUi((s) => s.pushToast);

  const [permissionState, setPermissionState] = useState<
    "waiting" | "requesting" | "ready" | "error"
  >("waiting");
  const [permissionError, setPermissionError] = useState<
    MediaError | undefined
  >(undefined);
  const [localStream, setLocalStream] = useState<MediaStream | null>(null);
  const [remoteFeeds, setRemoteFeeds] = useState<RemoteFeed[]>([]);
  const [micEnabled, setMicEnabled] = useState(true);
  const [cameraEnabled, setCameraEnabled] = useState(true);
  const [signalingState, setSignalingState] =
    useState<SignalingState>("idle");

  const signalingRef = useRef<SignalingClient | null>(null);
  const pcRef = useRef<PeerConnectionManager | null>(null);

  useEffect(() => {
    if (!session.joinToken || session.roomId !== params.id) {
      navigate(`/?next=/r/${params.id ?? ""}`);
    }
  }, [navigate, params.id, session.joinToken, session.roomId]);

  useEffect(
    () => () => {
      pcRef.current?.close();
      pcRef.current = null;
      signalingRef.current?.close();
      signalingRef.current = null;
      stopTracks(localStream);
    },
    [localStream],
  );

  const requestPermissions = useCallback(async () => {
    setPermissionState("requesting");
    const result = await requestMedia();
    if (result instanceof MediaStream) {
      setLocalStream(result);
      setPermissionState("ready");
    } else {
      setPermissionError(result);
      setPermissionState("error");
    }
  }, []);

  useEffect(() => {
    if (permissionState !== "ready" || !localStream) return;
    if (!session.joinToken || !session.wsUrl) return;
    if (signalingRef.current) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const signalingUrl = `${protocol}//${window.location.host}${session.wsUrl}`;

    const sig = new SignalingClient({
      url: signalingUrl,
      token: session.joinToken,
      onStateChange: setSignalingState,
    });
    signalingRef.current = sig;

    const pc = new PeerConnectionManager(sig);
    pcRef.current = pc;

    const peers = useSession.getState();
    const chat = useChat.getState();

    const detachJoined = sig.on("Joined", (ev) => {
      peers.setPeers(
        ev.peers.map((p) => ({
          pid: p.pid,
          displayName: p.display_name,
          ...(p.pubkey !== undefined ? { pubkey: p.pubkey } : {}),
        })),
      );
      for (const p of ev.peers) {
        if (p.pubkey) void recordPeerKey(p.pid, p.pubkey);
      }
      // Now that we know our pid, announce our pubkey.
      void announcePubkey(sig);
    });
    const detachPeerJoined = sig.on("PeerJoined", (ev) => {
      peers.addPeer({
        pid: ev.peer.pid,
        displayName: ev.peer.display_name,
        ...(ev.peer.pubkey !== undefined ? { pubkey: ev.peer.pubkey } : {}),
      });
      if (ev.peer.pubkey) void recordPeerKey(ev.peer.pid, ev.peer.pubkey);
    });
    const detachPeerUpdated = sig.on("PeerUpdated", (ev) => {
      // Mostly used for late-arriving pubkeys.
      if (ev.peer.pubkey) void recordPeerKey(ev.peer.pid, ev.peer.pubkey);
    });
    const detachPeerLeft = sig.on("PeerLeft", (ev) => {
      peers.removePeer(ev.pid);
      chat.removePeer(ev.pid);
      setRemoteFeeds((feeds) => feeds.filter((f) => f.pid !== ev.pid));
    });
    const detachChat = sig.on("Chat", (ev) => {
      const fromPeer = useSession.getState().peers.find(
        (p) => p.pid === ev.from,
      );
      void ingestChat({
        fromPid: ev.from,
        fromName: fromPeer?.displayName ?? ev.from.slice(0, 6),
        ciphertext: ev.ciphertext,
      });
    });
    const detachError = sig.on("Error", (ev) => {
      pushToast("error", `signaling error ${ev.code}: ${ev.message}`);
    });

    const detachTrack = pc.on((event) => {
      if (event.type === "track") {
        setRemoteFeeds((feeds) => {
          if (feeds.some((f) => f.pid === event.pid)) {
            return feeds.map((f) =>
              f.pid === event.pid ? { ...f, stream: event.stream } : f,
            );
          }
          return [...feeds, { pid: event.pid, stream: event.stream }];
        });
      }
    });

    sig.connect();
    pc.attachLocalAndOffer(localStream).catch((e: unknown) => {
      console.error("offer failed", e);
      pushToast("error", "couldn't start the call");
    });

    return () => {
      detachJoined();
      detachPeerJoined();
      detachPeerUpdated();
      detachPeerLeft();
      detachChat();
      detachError();
      detachTrack();
    };
  }, [
    permissionState,
    localStream,
    session.joinToken,
    session.wsUrl,
    pushToast,
  ]);

  const handleLeave = useCallback(() => {
    pcRef.current?.close();
    signalingRef.current?.close();
    stopTracks(localStream);
    void clearSessionKeys();
    useChat.getState().clear();
    session.clear();
    navigate("/");
  }, [localStream, navigate, session]);

  const toggleMic = useCallback(() => {
    setMicEnabled((prev) => {
      const next = !prev;
      pcRef.current?.setMicEnabled(localStream, next);
      return next;
    });
  }, [localStream]);

  const toggleCamera = useCallback(() => {
    setCameraEnabled((prev) => {
      const next = !prev;
      pcRef.current?.setCameraEnabled(localStream, next);
      return next;
    });
  }, [localStream]);

  const peers = useSession((s) => s.peers);

  const tiles = useMemo(() => {
    const result: Array<{
      pid: string;
      displayName: string;
      stream?: MediaStream;
      isSelf?: boolean;
    }> = [];
    if (localStream && session.participantId) {
      result.push({
        pid: session.participantId,
        displayName: session.displayName ?? "you",
        stream: localStream,
        isSelf: true,
      });
    }
    for (const p of peers) {
      const feed = remoteFeeds.find((f) => f.pid === p.pid);
      result.push({
        pid: p.pid,
        displayName: p.displayName,
        ...(feed?.stream !== undefined ? { stream: feed.stream } : {}),
      });
    }
    return result;
  }, [
    localStream,
    peers,
    remoteFeeds,
    session.displayName,
    session.participantId,
  ]);

  if (!session.joinToken || session.roomId !== params.id) {
    return null;
  }

  if (permissionState !== "ready") {
    return (
      <PermissionWizard
        state={permissionState}
        error={permissionError}
        onAllow={() => void requestPermissions()}
      />
    );
  }

  return (
    <main className="min-h-screen flex flex-col p-3 gap-3">
      <header className="flex items-center justify-between text-sm">
        <div>
          <h1 className="font-medium">Room {session.roomId}</h1>
          <p className="text-xs text-muted">
            ws: {signalingState} · {tiles.length}{" "}
            {tiles.length === 1 ? "tile" : "tiles"}
          </p>
        </div>
      </header>

      <div className="flex-1 min-h-0">
        <Grid tileCount={tiles.length}>
          {tiles.map((t) => (
            <VideoTile
              key={t.pid}
              pid={t.pid}
              displayName={t.displayName}
              stream={t.stream ?? undefined}
              isSelf={t.isSelf ?? false}
              micMuted={t.isSelf ? !micEnabled : false}
            />
          ))}
        </Grid>
      </div>

      <div className="flex justify-center">
        <Controls
          micEnabled={micEnabled}
          cameraEnabled={cameraEnabled}
          onToggleMic={toggleMic}
          onToggleCamera={toggleCamera}
          onLeave={handleLeave}
        />
      </div>

      <ChatPanel
        signaling={signalingRef.current}
        selfPid={session.participantId}
        selfName={session.displayName ?? "you"}
        peers={peers}
      />
    </main>
  );
}

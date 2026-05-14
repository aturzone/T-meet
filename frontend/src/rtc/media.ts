/** `getUserMedia` wrapper with structured error categories. */

export type MediaError =
  | { kind: "denied" }
  | { kind: "device-missing" }
  | { kind: "in-use" }
  | { kind: "unsupported" }
  | { kind: "other"; message: string };

export interface MediaPrefs {
  audio: boolean;
  video: boolean;
}

const DEFAULT_PREFS: MediaPrefs = { audio: true, video: true };

export async function requestMedia(
  prefs: MediaPrefs = DEFAULT_PREFS,
): Promise<MediaStream | MediaError> {
  if (typeof navigator === "undefined" || !navigator.mediaDevices) {
    return { kind: "unsupported" };
  }
  try {
    return await navigator.mediaDevices.getUserMedia({
      audio: prefs.audio,
      video: prefs.video
        ? {
            width: { ideal: 1280 },
            height: { ideal: 720 },
            frameRate: { ideal: 30, max: 30 },
          }
        : false,
    });
  } catch (e) {
    return classify(e);
  }
}

function classify(e: unknown): MediaError {
  const err = e as DOMException | undefined;
  switch (err?.name) {
    case "NotAllowedError":
    case "PermissionDeniedError":
    case "SecurityError":
      return { kind: "denied" };
    case "NotFoundError":
    case "DevicesNotFoundError":
      return { kind: "device-missing" };
    case "NotReadableError":
    case "TrackStartError":
      return { kind: "in-use" };
    default:
      return { kind: "other", message: err?.message ?? "unknown" };
  }
}

export function stopTracks(stream?: MediaStream | null): void {
  if (!stream) return;
  for (const t of stream.getTracks()) {
    t.stop();
  }
}

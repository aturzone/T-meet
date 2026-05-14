import { Camera, Mic } from "lucide-react";
import { Button } from "../ui/Button";
import { Card } from "../ui/Card";
import type { MediaError } from "../../rtc/media";

interface Props {
  state: "waiting" | "requesting" | "error";
  error?: MediaError | undefined;
  onAllow: () => void;
  onSkip?: () => void;
}

const messages: Record<MediaError["kind"], string> = {
  denied:
    "Browser permission was denied. Open the site settings and grant camera + microphone, then refresh.",
  "device-missing":
    "No camera or microphone was found. Plug one in and refresh.",
  "in-use":
    "Another app is using your camera or microphone. Close it and refresh.",
  unsupported:
    "This browser doesn't support getUserMedia. Try Brave or Chromium.",
  other: "Something went wrong starting your camera. Refresh to try again.",
};

export function PermissionWizard({
  state,
  error,
  onAllow,
  onSkip,
}: Props) {
  return (
    <div className="min-h-screen flex items-center justify-center p-6">
      <Card className="max-w-md text-center space-y-4">
        <div className="flex justify-center gap-4 text-muted">
          <Mic size={32} />
          <Camera size={32} />
        </div>
        <h1 className="text-xl font-semibold">
          Allow camera &amp; microphone
        </h1>
        {state === "error" && error ? (
          <p className="text-sm text-red-400">{messages[error.kind]}</p>
        ) : (
          <p className="text-sm text-muted">
            Your browser will ask for permission. Streams stay on this
            network — the server only forwards encrypted media.
          </p>
        )}
        <div className="flex gap-2 justify-center">
          <Button onClick={onAllow} loading={state === "requesting"}>
            Allow and join
          </Button>
          {onSkip && (
            <Button variant="ghost" onClick={onSkip}>
              Join without camera
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
}

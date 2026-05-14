import { Mic, MicOff, PhoneOff, Video, VideoOff } from "lucide-react";
import { Button } from "../ui/Button";

interface Props {
  micEnabled: boolean;
  cameraEnabled: boolean;
  onToggleMic: () => void;
  onToggleCamera: () => void;
  onLeave: () => void;
}

export function Controls({
  micEnabled,
  cameraEnabled,
  onToggleMic,
  onToggleCamera,
  onLeave,
}: Props) {
  return (
    <div className="flex items-center justify-center gap-2 p-2 rounded-lg bg-surface/80 backdrop-blur border border-border">
      <Button
        variant="ghost"
        onClick={onToggleMic}
        aria-pressed={!micEnabled}
        aria-label={micEnabled ? "mute microphone" : "unmute microphone"}
      >
        {micEnabled ? <Mic size={16} /> : <MicOff size={16} />}
      </Button>
      <Button
        variant="ghost"
        onClick={onToggleCamera}
        aria-pressed={!cameraEnabled}
        aria-label={cameraEnabled ? "turn camera off" : "turn camera on"}
      >
        {cameraEnabled ? <Video size={16} /> : <VideoOff size={16} />}
      </Button>
      <Button variant="danger" onClick={onLeave} aria-label="leave room">
        <PhoneOff size={16} />
      </Button>
    </div>
  );
}

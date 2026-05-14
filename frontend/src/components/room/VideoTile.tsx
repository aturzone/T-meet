import { useEffect, useRef } from "react";
import { MicOff } from "lucide-react";
import type { QualityLevel } from "../../rtc/quality";
import { cn } from "../../lib/cn";

interface Props {
  pid: string;
  displayName: string;
  stream?: MediaStream | undefined;
  isSelf?: boolean;
  micMuted?: boolean;
  quality?: QualityLevel | undefined;
  speaking?: boolean;
}

const qualityDot: Record<QualityLevel, string> = {
  good: "bg-emerald-400",
  ok: "bg-yellow-400",
  bad: "bg-red-500",
};

export function VideoTile({
  pid,
  displayName,
  stream,
  isSelf,
  micMuted,
  quality,
  speaking,
}: Props) {
  const ref = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (ref.current && stream) {
      ref.current.srcObject = stream;
    }
  }, [stream]);

  return (
    <div
      data-pid={pid}
      className={cn(
        "relative overflow-hidden rounded-lg border border-border bg-bg",
        "min-h-32 aspect-video",
        speaking && "ring-2 ring-accent",
      )}
    >
      <video
        ref={ref}
        autoPlay
        playsInline
        muted={isSelf || micMuted}
        className="absolute inset-0 w-full h-full object-cover bg-black"
      />
      <div className="absolute bottom-2 left-2 right-2 flex items-center gap-2 text-xs">
        <span className="bg-bg/80 px-2 py-1 rounded text-fg truncate flex-1">
          {displayName}
          {isSelf && " (you)"}
        </span>
        {micMuted && (
          <span
            className="bg-bg/80 p-1 rounded text-red-400"
            aria-label="microphone muted"
          >
            <MicOff size={12} />
          </span>
        )}
        {quality && (
          <span
            className={cn("size-2 rounded-full", qualityDot[quality])}
            aria-label={`quality ${quality}`}
          />
        )}
      </div>
    </div>
  );
}

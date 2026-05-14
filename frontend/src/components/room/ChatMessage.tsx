import type { ChatMessage as Msg } from "../../chat/store";
import { cn } from "../../lib/cn";

interface Props {
  msg: Msg;
  isSelf: boolean;
}

export function ChatMessage({ msg, isSelf }: Props) {
  const date = new Date(msg.ts);
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
  return (
    <div className={cn("flex flex-col text-sm", isSelf && "items-end")}>
      <div className="flex items-baseline gap-2">
        <span className="font-medium">
          {isSelf ? "you" : msg.fromName}
        </span>
        <span className="text-xs text-muted">{time}</span>
      </div>
      <p className="whitespace-pre-wrap break-words max-w-[42ch]">
        {msg.text}
      </p>
    </div>
  );
}

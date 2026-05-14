import { useEffect, useRef, useState } from "react";
import { Send, MessageSquare, X } from "lucide-react";

import { Button } from "../ui/Button";
import { useChat } from "../../chat/store";
import { sendChat } from "../../chat/wire";
import { MAX_PLAINTEXT_BYTES } from "../../chat/seal";
import { ChatMessage } from "./ChatMessage";
import type { SignalingClient } from "../../rtc/signaling";
import type { Peer } from "../../lib/store";

interface Props {
  signaling: SignalingClient | null;
  selfPid: string | undefined;
  selfName: string;
  peers: Peer[];
}

export function ChatPanel({ signaling, selfPid, selfName, peers }: Props) {
  const messages = useChat((s) => s.messages);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages]);

  async function handleSend() {
    if (!signaling || !selfPid || draft.trim().length === 0) return;
    setSending(true);
    try {
      await sendChat(signaling, draft, peers, selfPid, selfName);
      setDraft("");
    } finally {
      setSending(false);
    }
  }

  if (!open) {
    return (
      <Button
        variant="ghost"
        onClick={() => setOpen(true)}
        aria-label="open chat"
        className="fixed bottom-4 right-4"
      >
        <MessageSquare size={16} />
        {messages.length > 0 && (
          <span className="ml-2 rounded-full bg-accent text-bg text-xs px-2 py-0.5">
            {messages.length}
          </span>
        )}
      </Button>
    );
  }

  return (
    <aside className="fixed top-0 right-0 h-full w-full sm:w-80 border-l border-border bg-surface flex flex-col z-40">
      <header className="flex items-center justify-between p-3 border-b border-border">
        <h2 className="text-sm font-medium">Chat</h2>
        <button
          type="button"
          onClick={() => setOpen(false)}
          aria-label="close chat"
          className="text-muted hover:text-fg"
        >
          <X size={16} />
        </button>
      </header>

      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-3 space-y-3"
      >
        {messages.length === 0 ? (
          <p className="text-xs text-muted text-center mt-4">
            Messages are end-to-end encrypted and not saved.
          </p>
        ) : (
          messages.map((m) => (
            <ChatMessage
              key={m.id}
              msg={m}
              isSelf={m.fromPid === selfPid}
            />
          ))
        )}
      </div>

      <form
        className="border-t border-border p-2 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void handleSend();
        }}
      >
        <input
          aria-label="chat message"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          maxLength={MAX_PLAINTEXT_BYTES}
          placeholder="Type a message…"
          className="flex-1 h-9 px-3 rounded-md bg-bg text-fg border border-border placeholder:text-muted text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        />
        <Button
          type="submit"
          size="sm"
          loading={sending}
          disabled={draft.trim().length === 0}
          aria-label="send"
        >
          <Send size={14} />
        </Button>
      </form>
    </aside>
  );
}

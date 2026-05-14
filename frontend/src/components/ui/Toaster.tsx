import { useUi, type Toast } from "../../lib/store";
import { X } from "lucide-react";
import { cn } from "../../lib/cn";

const levelStyle: Record<Toast["level"], string> = {
  info: "border-border bg-surface text-fg",
  warn: "border-yellow-500/50 bg-yellow-500/10 text-yellow-100",
  error: "border-red-500/50 bg-red-500/10 text-red-100",
};

export function Toaster() {
  const toasts = useUi((s) => s.toasts);
  const dismiss = useUi((s) => s.dismissToast);

  return (
    <div
      className="fixed top-4 right-4 z-50 flex flex-col gap-2 max-w-sm"
      role="status"
      aria-live="polite"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            "flex items-start gap-3 rounded-md border px-4 py-3 text-sm shadow-lg",
            levelStyle[t.level],
          )}
        >
          <span className="flex-1">{t.message}</span>
          <button
            type="button"
            onClick={() => dismiss(t.id)}
            className="text-muted hover:text-fg"
            aria-label="dismiss"
          >
            <X size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}

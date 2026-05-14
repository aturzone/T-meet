import type { PropsWithChildren } from "react";
import { cn } from "../../lib/cn";

interface Props extends PropsWithChildren {
  tileCount: number;
  className?: string;
}

/** Layout-only grid. Picks the column count to keep tiles roughly square at
 *  N = 1, 2, 3, 4, 6, 9, 12, 16, 20. Beyond 20 the tiles get smaller and the
 *  grid flexes. */
export function Grid({ tileCount, className, children }: Props) {
  const cols = columnsForCount(tileCount);
  return (
    <div
      className={cn(
        "grid auto-rows-fr gap-2 w-full h-full",
        cols,
        className,
      )}
    >
      {children}
    </div>
  );
}

function columnsForCount(n: number): string {
  if (n <= 1) return "grid-cols-1";
  if (n <= 2) return "grid-cols-2";
  if (n <= 4) return "grid-cols-2";
  if (n <= 6) return "grid-cols-3";
  if (n <= 9) return "grid-cols-3";
  if (n <= 12) return "grid-cols-4";
  if (n <= 16) return "grid-cols-4";
  if (n <= 20) return "grid-cols-5";
  return "grid-cols-5";
}

import type { HTMLAttributes, PropsWithChildren } from "react";
import { cn } from "../../lib/cn";

interface Props extends HTMLAttributes<HTMLDivElement>, PropsWithChildren {}

export function Card({ className, children, ...rest }: Props) {
  return (
    <div
      className={cn(
        "rounded-lg border border-border bg-surface p-6 shadow-lg",
        className,
      )}
      {...rest}
    >
      {children}
    </div>
  );
}

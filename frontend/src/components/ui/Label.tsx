import type { LabelHTMLAttributes, PropsWithChildren } from "react";
import { cn } from "../../lib/cn";

interface Props
  extends LabelHTMLAttributes<HTMLLabelElement>,
    PropsWithChildren {}

export function Label({ className, children, ...rest }: Props) {
  return (
    <label
      className={cn("text-sm font-medium text-fg", className)}
      {...rest}
    >
      {children}
    </label>
  );
}

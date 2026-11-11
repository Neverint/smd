import type { ReactNode } from "react";

export type BadgeTone = "neutral" | "info" | "success" | "warning" | "danger";

export interface BadgeProps {
  tone?: BadgeTone;
  /** Screen-reader text when the visible content is only a number. */
  label?: string;
  children: ReactNode;
}

export function Badge({ tone = "info", label, children }: BadgeProps) {
  return (
    <span className={`smd-badge smd-badge--${tone}`} aria-label={label} role={label ? "status" : undefined}>
      {children}
    </span>
  );
}

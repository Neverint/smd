import { useId, type ReactNode } from "react";

export interface ProgressRingProps {
  /** 0-100. Leave undefined for an indeterminate spinner. */
  value?: number;
  size?: number;
  thickness?: number;
  /** Accessible name, e.g. "Scanning browser caches". */
  label: string;
  children?: ReactNode;
}

export function ProgressRing({ value, size = 160, thickness = 10, label, children }: ProgressRingProps) {
  const gradientId = useId();
  const radius = (size - thickness) / 2;
  const circumference = 2 * Math.PI * radius;
  const indeterminate = value === undefined;
  const clamped = indeterminate ? 25 : Math.max(0, Math.min(100, value));
  const offset = circumference * (1 - clamped / 100);

  return (
    <div
      className={`smd-ring${indeterminate ? " smd-ring--indeterminate" : ""}`}
      style={{ width: size, height: size }}
      role="progressbar"
      aria-label={label}
      aria-valuemin={indeterminate ? undefined : 0}
      aria-valuemax={indeterminate ? undefined : 100}
      aria-valuenow={indeterminate ? undefined : Math.round(clamped)}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--m-from, var(--accent))" />
            <stop offset="100%" stopColor="var(--m-to, var(--accent))" />
          </linearGradient>
        </defs>
        <circle className="smd-ring__track" cx={size / 2} cy={size / 2} r={radius} fill="none" strokeWidth={thickness} />
        <circle
          className="smd-ring__bar"
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={`url(#${gradientId})`}
          strokeWidth={thickness}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
      </svg>
      {children && <div className="smd-ring__center">{children}</div>}
    </div>
  );
}

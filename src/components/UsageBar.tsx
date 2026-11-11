export interface UsageBarProps {
  /** 0-100 */
  value: number;
  label: string;
  /** Turns the bar to the caution or critical color. */
  tone?: "normal" | "warning" | "critical";
}

/** Thin horizontal meter (drive usage, memory). */
export function UsageBar({ value, label, tone = "normal" }: UsageBarProps) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div
      className={`smd-usage smd-usage--${tone}`}
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v)}
    >
      <div className="smd-usage__fill" style={{ width: `${v}%` }} />
    </div>
  );
}

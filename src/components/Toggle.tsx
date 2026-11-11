export interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  /** Show the label next to the switch (otherwise it is only read by screen readers). */
  showLabel?: boolean;
  disabled?: boolean;
}

export function Toggle({ checked, onChange, label, showLabel = false, disabled }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={showLabel ? undefined : label}
      className="smd-toggle"
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="smd-toggle__track" aria-hidden="true">
        <span className="smd-toggle__thumb" />
      </span>
      {showLabel && <span>{label}</span>}
    </button>
  );
}

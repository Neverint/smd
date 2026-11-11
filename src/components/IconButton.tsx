import type { ButtonHTMLAttributes, ReactNode } from "react";

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  /** Required: read by screen readers and shown as a tooltip. */
  label: string;
  icon: ReactNode;
}

export function IconButton({ label, icon, className, type = "button", ...rest }: IconButtonProps) {
  return (
    <button type={type} aria-label={label} title={label} className={["smd-icon-btn", className].filter(Boolean).join(" ")} {...rest}>
      {icon}
    </button>
  );
}

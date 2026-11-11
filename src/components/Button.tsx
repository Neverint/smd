import type { ButtonHTMLAttributes, ReactNode } from "react";

export type ButtonVariant = "primary" | "accent" | "secondary" | "subtle" | "danger";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * primary = module color (one per screen), accent = system accent, secondary = neutral,
   * subtle = no fill, danger = confirms something that can't be undone.
   */
  variant?: ButtonVariant;
  size?: "md" | "lg";
  icon?: ReactNode;
}

export function Button({ variant = "secondary", size = "md", icon, className, children, type = "button", ...rest }: ButtonProps) {
  const classes = ["smd-btn", `smd-btn--${variant}`, size === "lg" && "smd-btn--lg", className].filter(Boolean).join(" ");
  return (
    <button type={type} className={classes} {...rest}>
      {icon}
      {children}
    </button>
  );
}

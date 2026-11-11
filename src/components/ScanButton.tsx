import { motion } from "motion/react";
import type { ButtonHTMLAttributes } from "react";

type NativeProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "onAnimationStart" | "onAnimationEnd" | "onAnimationIteration" | "onDrag" | "onDragStart" | "onDragEnd"
>;

export interface ScanButtonProps extends NativeProps {
  children: string;
}

/** The big round primary action of Smart Care. Uses the module gradient. */
export function ScanButton({ children, className, type = "button", ...rest }: ScanButtonProps) {
  return (
    <motion.button
      type={type}
      className={["smd-scan", className].filter(Boolean).join(" ")}
      whileHover={rest.disabled ? undefined : { scale: 1.04 }}
      whileTap={rest.disabled ? undefined : { scale: 0.97 }}
      transition={{ type: "spring", stiffness: 400, damping: 25 }}
      {...rest}
    >
      {children}
    </motion.button>
  );
}

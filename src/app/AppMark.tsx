import { useId } from "react";

/** Original app mark: a drive platter with a sparkle. Same artwork as app-icon.svg. */
export function AppMark({ size = 24 }: { size?: number }) {
  const id = useId();
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <defs>
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#14b8a6" />
          <stop offset="1" stopColor="#2563eb" />
        </linearGradient>
      </defs>
      <rect x="2" y="2" width="60" height="60" rx="16" fill={`url(#${id}-bg)`} />
      <circle cx="30" cy="34" r="17" fill="none" stroke="#fff" strokeWidth="4.5" strokeOpacity="0.95" />
      <circle cx="30" cy="34" r="4.5" fill="#fff" />
      <path d="M48 8 L50.6 15.4 L58 18 L50.6 20.6 L48 28 L45.4 20.6 L38 18 L45.4 15.4 Z" fill="#fff" />
    </svg>
  );
}

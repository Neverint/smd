import type { Language } from "../i18n";

/** Non-breaking space, so "12.4 GB" never wraps between number and unit. */
const NBSP = String.fromCharCode(0xa0);

const UNITS: Record<Language, string[]> = {
  en: ["B", "KB", "MB", "GB", "TB"],
  fr: ["o", "Ko", "Mo", "Go", "To"],
};

/**
 * Same rules as `smd_core::format::format_bytes`: 1024-based, one decimal under 100.
 * French uses Explorer's French units (Ko, Mo, Go) and a decimal comma.
 */
export function formatBytes(bytes: number, lang: Language = "en"): string {
  const units = UNITS[lang];
  const locale = lang === "fr" ? "fr-FR" : "en-US";
  if (!Number.isFinite(bytes) || bytes < 0) bytes = 0;
  if (bytes < 1024) return `${Math.round(bytes)}${NBSP}${units[0]}`;

  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  if (value >= 1023.95 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = value < 100 ? 1 : 0;
  const text = new Intl.NumberFormat(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(value);
  return `${text}${NBSP}${units[unit]}`;
}

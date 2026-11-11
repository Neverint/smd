import { useTranslation } from "react-i18next";
import type { Language } from "../i18n";
import { formatBytes } from "./format";

/** Formatting helpers that follow the UI language. */
export function useFormat() {
  const { i18n } = useTranslation();
  const lang: Language = i18n.language?.startsWith("fr") ? "fr" : "en";
  const locale = lang === "fr" ? "fr-FR" : "en-US";
  return {
    lang,
    bytes: (n: number) => formatBytes(n, lang),
    number: (n: number) => new Intl.NumberFormat(locale).format(n),
    percent: (n: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(n),
    seconds: (ms: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(ms / 1000),
    dateTime: (ms: number) => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(ms),
  };
}

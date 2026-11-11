import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";
import fr from "./fr.json";
import { useSettings, type LanguagePref } from "../stores/settings";

export type Language = "en" | "fr";
export const SUPPORTED_LANGUAGES: Language[] = ["en", "fr"];

/** "system" follows the Windows display language (WebView2 reports it as navigator.language). */
export function resolveLanguage(pref: LanguagePref, systemLanguage: string): Language {
  if (pref !== "system") return pref;
  return systemLanguage.toLowerCase().startsWith("fr") ? "fr" : "en";
}

function systemLanguage(): string {
  return typeof navigator === "undefined" ? "en" : navigator.language;
}

export function initI18n() {
  const lng = resolveLanguage(useSettings.getState().language, systemLanguage());
  void i18n.use(initReactI18next).init({
    resources: { en: { translation: en }, fr: { translation: fr } },
    lng,
    fallbackLng: "en",
    interpolation: { escapeValue: false },
    returnNull: false,
  });
  document.documentElement.lang = lng;

  useSettings.subscribe((state, prev) => {
    if (state.language === prev.language) return;
    const next = resolveLanguage(state.language, systemLanguage());
    void i18n.changeLanguage(next);
    document.documentElement.lang = next;
  });
  return i18n;
}

export default i18n;

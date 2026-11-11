import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSettings, type ThemePref } from "../stores/settings";
import { isTauri } from "./backend";

export type Theme = "light" | "dark";

export function resolveTheme(pref: ThemePref, systemDark: boolean): Theme {
  if (pref === "system") return systemDark ? "dark" : "light";
  return pref;
}

const DARK_QUERY = "(prefers-color-scheme: dark)";

function useSystemDark(): boolean {
  const [dark, setDark] = useState(() => window.matchMedia(DARK_QUERY).matches);
  useEffect(() => {
    const mq = window.matchMedia(DARK_QUERY);
    const onChange = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return dark;
}

/**
 * Applies the theme to <html data-theme> and to the native window,
 * so the Mica tint and the title bar match the page.
 */
export function useApplyTheme(): Theme {
  const pref = useSettings((s) => s.theme);
  const systemDark = useSystemDark();
  const theme = resolveTheme(pref, systemDark);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    if (!isTauri()) return;
    void getCurrentWindow()
      .setTheme(pref === "system" ? null : pref)
      .catch(() => undefined);
  }, [pref]);

  return theme;
}

import { create } from "zustand";
import { persist } from "zustand/middleware";

export type ThemePref = "system" | "light" | "dark";
export type LanguagePref = "system" | "en" | "fr";

export interface SettingsState {
  theme: ThemePref;
  language: LanguagePref;
  sidebarCollapsed: boolean;
  setTheme: (theme: ThemePref) => void;
  setLanguage: (language: LanguagePref) => void;
  toggleSidebar: () => void;
}

export const DEFAULT_SETTINGS = {
  theme: "system",
  language: "system",
  sidebarCollapsed: false,
} as const satisfies Pick<SettingsState, "theme" | "language" | "sidebarCollapsed">;

/**
 * UI preferences. Phase 0 keeps them in browser storage; settings the backend
 * needs (exclusions, schedule, permanent delete) will live in a backend file.
 */
export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      ...DEFAULT_SETTINGS,
      setTheme: (theme) => set({ theme }),
      setLanguage: (language) => set({ language }),
      toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
    }),
    { name: "smd.settings", version: 1 },
  ),
);

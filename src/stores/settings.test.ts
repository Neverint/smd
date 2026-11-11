import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, useSettings } from "./settings";
import { resolveLanguage } from "../i18n";
import { resolveTheme } from "../lib/theme";

describe("settings store", () => {
  beforeEach(() => useSettings.setState(DEFAULT_SETTINGS));

  it("starts with system defaults", () => {
    const s = useSettings.getState();
    expect(s.theme).toBe("system");
    expect(s.language).toBe("system");
    expect(s.sidebarCollapsed).toBe(false);
  });

  it("updates and persists preferences", () => {
    useSettings.getState().setTheme("dark");
    useSettings.getState().setLanguage("fr");
    useSettings.getState().toggleSidebar();
    const s = useSettings.getState();
    expect([s.theme, s.language, s.sidebarCollapsed]).toEqual(["dark", "fr", true]);
    const saved = JSON.parse(localStorage.getItem("smd.settings") ?? "{}");
    expect(saved.state.theme).toBe("dark");
  });
});

describe("resolveLanguage", () => {
  it("follows the system language when set to system", () => {
    expect(resolveLanguage("system", "fr-FR")).toBe("fr");
    expect(resolveLanguage("system", "fr-CA")).toBe("fr");
    expect(resolveLanguage("system", "en-GB")).toBe("en");
    expect(resolveLanguage("system", "de-DE")).toBe("en");
  });

  it("uses an explicit choice", () => {
    expect(resolveLanguage("en", "fr-FR")).toBe("en");
    expect(resolveLanguage("fr", "en-US")).toBe("fr");
  });
});

describe("resolveTheme", () => {
  it("resolves system against the OS setting", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
  });
});

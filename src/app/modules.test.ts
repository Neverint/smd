import { describe, expect, it } from "vitest";
import en from "../i18n/en.json";
import fr from "../i18n/fr.json";
import { MODULES, moduleForPath } from "./modules";

function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) =>
    v && typeof v === "object" ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`],
  );
}

describe("module registry", () => {
  it("has unique ids and paths", () => {
    expect(new Set(MODULES.map((m) => m.id)).size).toBe(MODULES.length);
    expect(new Set(MODULES.map((m) => m.path)).size).toBe(MODULES.length);
  });

  it("maps / to Smart Care", () => {
    expect(moduleForPath("/")?.id).toBe("smart-care");
    expect(moduleForPath("/nope")).toBeUndefined();
  });

  it("has a name and tagline for every module in both languages", () => {
    for (const m of MODULES) {
      for (const dict of [en, fr] as unknown as Record<string, Record<string, Record<string, string>>>[]) {
        expect(dict.modules[m.id]?.name, m.id).toBeTruthy();
        expect(dict.modules[m.id]?.tagline, m.id).toBeTruthy();
      }
    }
  });
});

describe("translations", () => {
  it("French and English have the same keys", () => {
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });
});

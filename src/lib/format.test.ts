import { describe, expect, it } from "vitest";
import { formatBytes } from "./format";

const nb = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");

describe("formatBytes", () => {
  it("formats small values in bytes", () => {
    expect(nb(formatBytes(0))).toBe("0 B");
    expect(nb(formatBytes(1023))).toBe("1023 B");
  });

  it("matches the Rust formatter for English", () => {
    expect(nb(formatBytes(1536))).toBe("1.5 KB");
    expect(nb(formatBytes(12_884_901_888))).toBe("12.0 GB");
    expect(nb(formatBytes(238 * 1024 ** 3))).toBe("238 GB");
    expect(nb(formatBytes(1024 * 1024 - 1))).toBe("1.0 MB");
  });

  it("uses French units and a decimal comma", () => {
    expect(nb(formatBytes(1536, "fr"))).toBe("1,5 Ko");
    expect(nb(formatBytes(12_884_901_888, "fr"))).toBe("12,0 Go");
  });

  it("treats invalid input as zero", () => {
    expect(nb(formatBytes(-5))).toBe("0 B");
    expect(nb(formatBytes(Number.NaN))).toBe("0 B");
  });
});

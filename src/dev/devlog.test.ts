// DEVLOG: tests for the developer data log (removed with it).
import { describe, expect, it } from "vitest";
import { summarize } from "./devlog";

describe("devlog summarize", () => {
  it("cuts long strings and long arrays", () => {
    const long = "x".repeat(500);
    expect(summarize(long)).toMatch(/…\(500\)$/);
    expect(summarize([1, 2, 3, 4, 5, 6, 7])).toEqual({ len: 7, first: [1, 2, 3] });
    expect(summarize({ paths: ["a", "b"], n: 1 })).toEqual({ paths: ["a", "b"], n: 1 });
  });

  it("keeps channels as their id", () => {
    const channel = { id: 4, toJSON: () => "__CHANNEL__:4" };
    expect(summarize({ onProgress: channel })).toEqual({ onProgress: "__CHANNEL__:4" });
  });
});

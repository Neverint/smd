import { beforeEach, describe, expect, it } from "vitest";
import { resetMocks } from "../lib/mocks";
import type { CleanupCategory, CleanupScanResult } from "../lib/types";
import { buildSelections, initialSelection, isSelectable, selectedBytes, useCleanup } from "./cleanup";

function cat(id: string, extra: Partial<CleanupCategory> = {}): CleanupCategory {
  return {
    id,
    group: "system",
    safety: "safe",
    admin: false,
    permanent: false,
    defaultSelected: true,
    bytes: 300,
    files: 2,
    items: [
      { path: `C:\\${id}\\a`, name: "a", bytes: 100, files: 1, modifiedMs: null, isDir: false },
      { path: `C:\\${id}\\b`, name: "b", bytes: 200, files: 1, modifiedMs: null, isDir: false },
    ],
    excluded: 0,
    running: [],
    unreadable: false,
    moreItems: 0,
    moreBytes: 0,
    ...extra,
  };
}

const scan: CleanupScanResult = {
  scanId: 7,
  reclaimableBytes: 600,
  durationMs: 10,
  categories: [
    cat("temp"),
    cat("thumbs", { defaultSelected: false }),
    cat("admin", { admin: true }),
    cat("empty", { bytes: 0, items: [] }),
    cat("bin", { permanent: true, defaultSelected: false, items: [], bytes: 5000 }),
  ],
};

describe("cleanup selection", () => {
  it("pre-selects only default, non-admin categories that found something", () => {
    expect(initialSelection(scan)).toEqual({ temp: true, thumbs: false, admin: false, empty: false, bin: false });
    expect(isSelectable(scan.categories[2])).toBe(false);
    expect(isSelectable(scan.categories[3])).toBe(false);
  });

  it("counts selected bytes minus skipped items, never admin ones", () => {
    const selected = { temp: true, thumbs: true, admin: true, bin: true };
    expect(selectedBytes(scan, selected, {})).toBe(300 + 300 + 5000);
    expect(selectedBytes(scan, selected, { temp: ["C:\\temp\\b"] })).toBe(100 + 300 + 5000);
    expect(selectedBytes(null, selected, {})).toBe(0);
  });

  it("builds the run request with skipped paths", () => {
    const sel = buildSelections(scan, { temp: true, admin: true, bin: true }, { temp: ["C:\\temp\\a"] });
    expect(sel).toEqual([
      { id: "temp", skip: ["C:\\temp\\a"] },
      { id: "bin", skip: [] },
    ]);
  });
});

describe("cleanup store with the mocked backend", () => {
  beforeEach(() => {
    resetMocks();
    useCleanup.getState().reset();
    (window as unknown as { __SMD_FAST_MOCKS__: boolean }).__SMD_FAST_MOCKS__ = true;
  });

  it("scans, runs and refuses to run the same scan twice", async () => {
    await useCleanup.getState().startScan();
    const s = useCleanup.getState();
    expect(s.phase).toBe("results");
    expect(s.scan?.reclaimableBytes).toBeGreaterThan(0);
    const report = await s.run();
    expect(report?.removedBytes).toBe(selectedBytes(s.scan, s.selected, s.skipped));
    expect(useCleanup.getState().phase).toBe("done");
    expect(useCleanup.getState().scan).toBeNull();
  });
});

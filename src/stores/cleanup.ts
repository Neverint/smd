import { create } from "zustand";
import { call, stream, toBackendError, type BackendError } from "../lib/backend";
import type { CleanupCategory, CleanupReport, CleanupScanResult, CleanupSelection, ScanProgress } from "../lib/types";

export type CleanupPhase = "idle" | "scanning" | "results" | "running" | "done" | "error";

interface CleanupState {
  phase: CleanupPhase;
  progress: ScanProgress | null;
  scan: CleanupScanResult | null;
  /** Category id → ticked. */
  selected: Record<string, boolean>;
  /** Category id → item paths the user unticked. */
  skipped: Record<string, string[]>;
  report: CleanupReport | null;
  error: BackendError | null;

  startScan: () => Promise<void>;
  /** Use results from another scan (Smart Care runs the same rules). */
  adoptScan: (scan: CleanupScanResult) => void;
  toggleCategory: (id: string) => void;
  setSkipped: (id: string, paths: string[]) => void;
  run: () => Promise<CleanupReport | null>;
  reset: () => void;
}

/** A category can be selected when it needs no admin rights and has something in it. */
export function isSelectable(c: CleanupCategory): boolean {
  return !c.admin && c.bytes > 0;
}

export function initialSelection(scan: CleanupScanResult): Record<string, boolean> {
  return Object.fromEntries(scan.categories.map((c) => [c.id, isSelectable(c) && c.defaultSelected]));
}

/** Bytes that a run would free with the current selection. */
export function selectedBytes(scan: CleanupScanResult | null, selected: Record<string, boolean>, skipped: Record<string, string[]>): number {
  if (!scan) return 0;
  let total = 0;
  for (const c of scan.categories) {
    if (!selected[c.id] || !isSelectable(c)) continue;
    const skip = new Set(skipped[c.id] ?? []);
    const skippedBytes = c.items.filter((i) => skip.has(i.path)).reduce((s, i) => s + i.bytes, 0);
    total += c.bytes - skippedBytes;
  }
  return total;
}

export function buildSelections(scan: CleanupScanResult, selected: Record<string, boolean>, skipped: Record<string, string[]>): CleanupSelection[] {
  return scan.categories.filter((c) => selected[c.id] && isSelectable(c)).map((c) => ({ id: c.id, skip: skipped[c.id] ?? [] }));
}

export const useCleanup = create<CleanupState>()((set, get) => ({
  phase: "idle",
  progress: null,
  scan: null,
  selected: {},
  skipped: {},
  report: null,
  error: null,

  startScan: async () => {
    set({ phase: "scanning", progress: null, error: null, report: null });
    try {
      const scan = await stream("cleanup_scan", {}, (progress) => set({ progress }));
      get().adoptScan(scan);
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  adoptScan: (scan) => set({ phase: "results", scan, selected: initialSelection(scan), skipped: {}, report: null, error: null }),

  toggleCategory: (id) => set((s) => ({ selected: { ...s.selected, [id]: !s.selected[id] } })),

  setSkipped: (id, paths) => set((s) => ({ skipped: { ...s.skipped, [id]: paths } })),

  run: async () => {
    const { scan, selected, skipped } = get();
    if (!scan) return null;
    set({ phase: "running", error: null });
    try {
      const report = await call("cleanup_run", { scanId: scan.scanId, selections: buildSelections(scan, selected, skipped) });
      set({ phase: "done", report, scan: null });
      return report;
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
      return null;
    }
  },

  reset: () => set({ phase: "idle", progress: null, scan: null, selected: {}, skipped: {}, report: null, error: null }),
}));

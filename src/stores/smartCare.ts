import { create } from "zustand";
import { call, stream, toBackendError, type BackendError } from "../lib/backend";
import type { CleanupReport, ScanProgress, SmartCareResult } from "../lib/types";
import { buildSelections, initialSelection, useCleanup } from "./cleanup";

export type SmartCarePhase = "idle" | "scanning" | "results" | "running" | "done" | "error";

interface SmartCareState {
  phase: SmartCarePhase;
  progress: ScanProgress | null;
  result: SmartCareResult | null;
  report: CleanupReport | null;
  error: BackendError | null;
  startScan: () => Promise<void>;
  /** Cleans the junk categories ticked by default (Recycle Bin and admin items excluded). */
  runCleanup: () => Promise<void>;
  reset: () => void;
}

export const useSmartCare = create<SmartCareState>()((set, get) => ({
  phase: "idle",
  progress: null,
  result: null,
  report: null,
  error: null,

  startScan: async () => {
    set({ phase: "scanning", progress: null, error: null, report: null });
    try {
      const result = await stream("smart_care_scan", {}, (progress) => set({ progress }));
      useCleanup.getState().adoptScan(result.cleanup);
      set({ phase: "results", result });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  runCleanup: async () => {
    const scan = get().result?.cleanup;
    if (!scan) return;
    set({ phase: "running", error: null });
    try {
      const selections = buildSelections(scan, initialSelection(scan), {});
      const report = await call("cleanup_run", { scanId: scan.scanId, selections });
      // The scan is used up; Cleanup starts fresh too.
      useCleanup.getState().reset();
      set({ phase: "done", report });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  reset: () => set({ phase: "idle", progress: null, result: null, report: null, error: null }),
}));

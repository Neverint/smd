import { create } from "zustand";
import { call, stream, toBackendError, type BackendError } from "../lib/backend";
import type { ClutterProgress, ClutterView, RemovalReport } from "../lib/types";

export type ClutterPhase = "idle" | "scanning" | "results" | "removing" | "done" | "error";

interface ClutterState {
  phase: ClutterPhase;
  progress: ClutterProgress | null;
  view: ClutterView | null;
  report: RemovalReport | null;
  error: BackendError | null;
  scan: (path: string | null) => Promise<void>;
  pickAndScan: () => Promise<void>;
  cancel: () => Promise<void>;
  remove: (scanId: number, paths: string[]) => Promise<void>;
  reset: () => void;
}

/** Duplicate files to select so that one copy per group stays: all but the first (shortest path). */
export function allButOne(groups: { files: { path: string }[] }[]): string[] {
  return groups.flatMap((g) => g.files.slice(1).map((f) => f.path));
}

export const useClutter = create<ClutterState>()((set, get) => ({
  phase: "idle",
  progress: null,
  view: null,
  report: null,
  error: null,

  scan: async (path) => {
    set({ phase: "scanning", progress: null, view: null, report: null, error: null });
    try {
      const view = await stream("clutter_scan", { path }, (progress) => set({ progress }));
      set(view.result.cancelled ? { phase: "idle", progress: null } : { phase: "results", view });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  pickAndScan: async () => {
    try {
      const path = await call("pick_folder");
      if (path) await get().scan(path);
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  cancel: async () => {
    await call("clutter_cancel").catch(() => undefined);
  },

  remove: async (scanId, paths) => {
    set({ phase: "removing", error: null });
    try {
      const report = await call("clutter_remove", { scanId, paths });
      set({ phase: "done", report, view: null });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  reset: () => set({ phase: "idle", progress: null, view: null, report: null, error: null }),
}));

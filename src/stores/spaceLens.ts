import { create } from "zustand";
import { call, stream, toBackendError, type BackendError } from "../lib/backend";
import type { DriveInfo, SpaceProgress, SpaceSummary, SpaceView } from "../lib/types";

export type SpacePhase = "idle" | "scanning" | "results" | "error";

interface SpaceLensState {
  phase: SpacePhase;
  drives: DriveInfo[];
  target: string | null;
  progress: SpaceProgress | null;
  summary: SpaceSummary | null;
  view: SpaceView | null;
  error: BackendError | null;
  loadDrives: () => Promise<void>;
  scan: (path: string) => Promise<void>;
  pickFolder: () => Promise<void>;
  cancel: () => Promise<void>;
  open: (nodeId: number) => Promise<void>;
  reveal: (nodeId: number) => Promise<void>;
  reset: () => void;
}

export const useSpaceLens = create<SpaceLensState>()((set, get) => ({
  phase: "idle",
  drives: [],
  target: null,
  progress: null,
  summary: null,
  view: null,
  error: null,

  loadDrives: async () => {
    try {
      set({ drives: await call("drives_list") });
    } catch (err) {
      set({ error: toBackendError(err) });
    }
  },

  scan: async (path) => {
    set({ phase: "scanning", target: path, progress: null, summary: null, view: null, error: null });
    try {
      const summary = await stream("space_scan", { path }, (progress) => set({ progress }));
      if (summary.cancelled) {
        set({ phase: "idle", progress: null });
        return;
      }
      set({ phase: "results", summary, view: summary.view });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  pickFolder: async () => {
    try {
      const path = await call("pick_folder");
      if (path) await get().scan(path);
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  cancel: async () => {
    await call("space_cancel").catch(() => undefined);
  },

  open: async (nodeId) => {
    const scanId = get().view?.scanId;
    if (scanId === undefined) return;
    try {
      set({ view: await call("space_view", { scanId, nodeId }) });
    } catch (err) {
      set({ phase: "error", error: toBackendError(err) });
    }
  },

  reveal: async (nodeId) => {
    const scanId = get().view?.scanId;
    if (scanId === undefined) return;
    try {
      await call("space_reveal", { scanId, nodeId });
    } catch (err) {
      set({ error: toBackendError(err) });
    }
  },

  reset: () => set({ phase: "idle", target: null, progress: null, summary: null, view: null, error: null }),
}));

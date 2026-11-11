import { Channel, invoke } from "@tauri-apps/api/core";
import { mockHandlers, mockStreams } from "./mocks";
import type {
  ActivityEntry,
  AppInfo,
  BackendSettings,
  CleanupReport,
  CleanupScanResult,
  CleanupSelection,
  DriveInfo,
  RestoreReport,
  ScanProgress,
  SmartCareResult,
  SpaceProgress,
  SpaceSummary,
  SpaceView,
} from "./types";

export type * from "./types";

/** True inside the Tauri window; false in a plain browser (Vite dev server, Playwright, gallery). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Shape of every backend error (see src-tauri/src/error.rs). */
export interface BackendError {
  code: "accessDenied" | "notFound" | "unsupported" | "io" | "stale" | "busy" | "internal" | "unknown";
  message: string;
  details: string;
}

type NoArgs = Record<string, never>;

/** Every request/response backend command. Keep in sync with lib.rs. */
export interface Commands {
  app_info: { args: NoArgs; result: AppInfo };
  format_bytes: { args: { bytes: number }; result: string };
  settings_get: { args: NoArgs; result: BackendSettings };
  settings_set: { args: { settings: BackendSettings }; result: BackendSettings };
  open_log_folder: { args: NoArgs; result: null };
  activity_list: { args: NoArgs; result: ActivityEntry[] };
  activity_undo: { args: { id: number }; result: RestoreReport };
  cleanup_run: { args: { scanId: number; selections: CleanupSelection[] }; result: CleanupReport };
  drives_list: { args: NoArgs; result: DriveInfo[] };
  pick_folder: { args: NoArgs; result: string | null };
  space_cancel: { args: NoArgs; result: null };
  space_view: { args: { scanId: number; nodeId: number }; result: SpaceView };
  space_reveal: { args: { scanId: number; nodeId: number }; result: null };
}

/** Commands that stream progress through a channel (`on_progress` in Rust). */
export interface StreamCommands {
  cleanup_scan: { args: NoArgs; progress: ScanProgress; result: CleanupScanResult };
  smart_care_scan: { args: NoArgs; progress: ScanProgress; result: SmartCareResult };
  space_scan: { args: { path: string }; progress: SpaceProgress; result: SpaceSummary };
}

export type CommandName = keyof Commands;
export type StreamName = keyof StreamCommands;

export type MockHandlers = {
  [K in CommandName]: (args: Commands[K]["args"]) => Commands[K]["result"] | Promise<Commands[K]["result"]>;
};
export type MockStreams = {
  [K in StreamName]: (
    args: StreamCommands[K]["args"],
    onProgress: (p: StreamCommands[K]["progress"]) => void,
  ) => Promise<StreamCommands[K]["result"]>;
};

export function toBackendError(err: unknown): BackendError {
  if (err && typeof err === "object" && "code" in err && "message" in err) {
    return err as BackendError;
  }
  const message = err instanceof Error ? err.message : String(err);
  return { code: "unknown", message, details: message };
}

/** Typed call into the Rust backend. Uses mocks outside Tauri. Throws a BackendError. */
export async function call<K extends CommandName>(
  cmd: K,
  ...args: Commands[K]["args"] extends NoArgs ? [] : [Commands[K]["args"]]
): Promise<Commands[K]["result"]> {
  const payload = (args[0] ?? {}) as Commands[K]["args"];
  try {
    if (!isTauri()) return await mockHandlers[cmd](payload);
    return await invoke<Commands[K]["result"]>(cmd, payload);
  } catch (err) {
    throw toBackendError(err);
  }
}

/** Like `call`, for long tasks that report progress. */
export async function stream<K extends StreamName>(
  cmd: K,
  args: StreamCommands[K]["args"],
  onProgress: (p: StreamCommands[K]["progress"]) => void,
): Promise<StreamCommands[K]["result"]> {
  try {
    if (!isTauri()) return await mockStreams[cmd](args, onProgress);
    const channel = new Channel<StreamCommands[K]["progress"]>();
    channel.onmessage = onProgress;
    return await invoke<StreamCommands[K]["result"]>(cmd, { ...args, onProgress: channel });
  } catch (err) {
    throw toBackendError(err);
  }
}

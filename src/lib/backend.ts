import { invoke } from "@tauri-apps/api/core";
import { mockHandlers } from "./mocks";

/** True inside the Tauri window; false in a plain browser (Vite dev server, Playwright, gallery). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export type WindowEffect = "mica" | "acrylic" | "none";

export interface AppInfo {
  version: string;
  windowEffect: WindowEffect;
}

/** Shape of every backend error (see src-tauri/src/error.rs). */
export interface BackendError {
  code: "accessDenied" | "notFound" | "unsupported" | "io" | "internal" | "unknown";
  message: string;
  details: string;
}

/** Every backend command with its arguments and result type. Keep in sync with commands.rs. */
export interface Commands {
  app_info: { args: Record<string, never>; result: AppInfo };
  format_bytes: { args: { bytes: number }; result: string };
}

export type CommandName = keyof Commands;
export type MockHandlers = {
  [K in CommandName]: (args: Commands[K]["args"]) => Commands[K]["result"] | Promise<Commands[K]["result"]>;
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
  ...args: Commands[K]["args"] extends Record<string, never> ? [] : [Commands[K]["args"]]
): Promise<Commands[K]["result"]> {
  const payload = (args[0] ?? {}) as Commands[K]["args"];
  try {
    if (!isTauri()) return await mockHandlers[cmd](payload);
    return await invoke<Commands[K]["result"]>(cmd, payload);
  } catch (err) {
    throw toBackendError(err);
  }
}

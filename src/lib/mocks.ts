import type { MockHandlers } from "./backend";
import { formatBytes } from "./format";

/** Fake backend used outside Tauri (browser dev, Playwright, gallery). No real system access. */
export const mockHandlers: MockHandlers = {
  app_info: () => ({ version: "0.1.0-mock", windowEffect: "none" }),
  format_bytes: ({ bytes }) => formatBytes(bytes, "en").replace(/\s/u, " "),
};

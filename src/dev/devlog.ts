/**
 * DEVLOG: developer data log, for the road to 1.0 only. See docs/DEVLOG.md.
 *
 * Records what happens in the UI into the backend's session file (src-tauri/src/devlog.rs):
 * every backend command with its time and error, pages, clicks, UI errors and window focus.
 * Only inside the Tauri app; nothing leaves the PC. Removed with `npm run devlog:remove`.
 */

import { invoke } from "@tauri-apps/api/core";

type Json = Record<string, unknown>;

interface TauriInternals {
  metadata?: { currentWindow?: { label?: string } };
}

/** Polled every second or so: counted per minute instead of one line per call. */
const POLLED = new Set(["tray_sample", "sensors_poll", "sensors_admin_poll", "processes_snapshot", "tray_info"]);
const FLUSH_MS = 2000;

const queue: Json[] = [];
const polls = new Map<string, { n: number; errors: number; maxMs: number }>();
let flushing = false;
let installed = false;

function push(k: string, data: Json = {}) {
  queue.push({ k, t: Date.now(), route: location.hash || "#/", ...data });
}

/** Short, safe view of command arguments: strings cut, arrays as their length and first items. */
export function summarize(value: unknown, depth = 0): unknown {
  if (value === null || value === undefined) return value;
  if (typeof value === "string") return value.length > 160 ? `${value.slice(0, 160)}…(${value.length})` : value;
  if (typeof value !== "object") return value;
  if (depth > 2) return "…";
  if (Array.isArray(value)) {
    return value.length > 5 ? { len: value.length, first: value.slice(0, 3).map((v) => summarize(v, depth + 1)) } : value.map((v) => summarize(v, depth + 1));
  }
  // Tauri channels (progress callbacks) serialize to "__CHANNEL__:<id>": keep just that.
  const toJSON = (value as { toJSON?: unknown }).toJSON;
  if (typeof toJSON === "function") return String(toJSON.call(value));
  const out: Json = {};
  for (const [key, v] of Object.entries(value as Json)) out[key] = summarize(v, depth + 1);
  return out;
}

/** Size of a result, without serializing big ones. */
function resultShape(r: unknown): unknown {
  if (Array.isArray(r)) return { len: r.length };
  if (r && typeof r === "object") return { keys: Object.keys(r).length };
  return typeof r === "string" && r.length > 80 ? { strLen: r.length } : r;
}

function errorInfo(e: unknown): Json {
  if (e && typeof e === "object") {
    const o = e as Json;
    return { code: o.code, message: String(o.message ?? e), details: typeof o.details === "string" ? o.details.slice(0, 2000) : undefined };
  }
  return { message: String(e) };
}

async function flush() {
  if (!installed || flushing) return;
  for (const [cmd, p] of polls) {
    queue.push({ k: "polls", t: Date.now(), cmd, ...p });
  }
  polls.clear();
  if (queue.length === 0) return;
  const events = queue.splice(0, queue.length);
  flushing = true;
  try {
    await invoke("devlog_write", { events });
  } catch {
    // Older backend without the command, or shutting down: drop the batch.
  } finally {
    flushing = false;
  }
}

function label(el: Element): string {
  const aria = el.getAttribute("aria-label");
  const text = aria ?? (el as HTMLElement).innerText ?? el.textContent ?? "";
  return text.replace(/\s+/g, " ").trim().slice(0, 80);
}

/**
 * Records one backend command (called from `call` and `stream` in src/lib/backend.ts).
 * Only watches the promise; the caller gets it unchanged.
 */
export function devlogCommand(cmd: string, args: unknown, result: Promise<unknown>) {
  if (!installed) return;
  const started = performance.now();
  const polled = POLLED.has(cmd);
  const done = (ok: boolean, value: unknown) => {
    const ms = Math.round(performance.now() - started);
    if (polled) {
      const p = polls.get(cmd) ?? { n: 0, errors: 0, maxMs: 0 };
      p.n += 1;
      p.maxMs = Math.max(p.maxMs, ms);
      if (!ok) p.errors += 1;
      polls.set(cmd, p);
      if (ok) return;
    }
    push("cmd", ok ? { cmd, ms, ok, args: summarize(args), result: resultShape(value) } : { cmd, ms, ok, args: summarize(args), error: errorInfo(value) });
  };
  result.then(
    (v) => done(true, v),
    (e) => done(false, e),
  );
}

function install() {
  const internals = (window as unknown as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__;
  if (!internals || installed) return;
  installed = true;

  const windowLabel = internals.metadata?.currentWindow?.label ?? "main";
  push("open", {
    window: windowLabel,
    size: `${window.innerWidth}x${window.innerHeight}`,
    dpr: window.devicePixelRatio,
    lang: navigator.language,
    dark: matchMedia("(prefers-color-scheme: dark)").matches,
    reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
  });

  window.addEventListener("hashchange", () => push("page"));
  window.addEventListener(
    "click",
    (ev) => {
      const el = (ev.target as Element | null)?.closest("button, a, [role=button], [role=tab], [role=menuitem], [role=checkbox], [role=switch], input, select, summary, label");
      if (!el) return;
      const input = el as HTMLInputElement;
      push("click", {
        el: el.tagName.toLowerCase(),
        role: el.getAttribute("role") ?? undefined,
        label: label(el),
        testid: el.getAttribute("data-testid") ?? undefined,
        checked: input.type === "checkbox" ? input.checked : undefined,
        disabled: (el as HTMLButtonElement).disabled || el.getAttribute("aria-disabled") === "true" || undefined,
      });
    },
    { capture: true },
  );
  window.addEventListener("error", (ev) => push("error", { message: ev.message, at: `${ev.filename}:${ev.lineno}:${ev.colno}`, stack: ev.error?.stack?.slice(0, 4000) }));
  window.addEventListener("unhandledrejection", (ev) => push("rejection", errorInfo(ev.reason)));
  document.addEventListener("visibilitychange", () => push("visibility", { visible: document.visibilityState === "visible" }));
  window.addEventListener("focus", () => push("focus"));
  window.addEventListener("pagehide", () => void flush());

  for (const level of ["error", "warn"] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      push("console", { level, msg: args.map((a) => (a instanceof Error ? `${a.message}\n${a.stack ?? ""}` : typeof a === "string" ? a : JSON.stringify(summarize(a)))).join(" ").slice(0, 4000) });
      original(...args);
    };
  }

  // Slow frames (the UI froze for over 200 ms), where the browser reports them.
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        if (e.duration >= 200) push("longtask", { ms: Math.round(e.duration) });
      }
    }).observe({ type: "longtask", buffered: true });
  } catch {
    // Not supported: skip.
  }
  window.addEventListener("load", () => {
    const nav = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
    const paint = performance.getEntriesByName("first-contentful-paint")[0];
    push("load", { domMs: nav ? Math.round(nav.domContentLoadedEventEnd) : undefined, fcpMs: paint ? Math.round(paint.startTime) : undefined });
  });

  setInterval(() => void flush(), FLUSH_MS);
}

install();

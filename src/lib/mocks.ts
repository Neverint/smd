import type { MockHandlers, MockStreams } from "./backend";
import { formatBytes } from "./format";
import { phase2Handlers, phase2Streams, resetPhase2Mocks } from "./mocks-phase2";
import { phase3Handlers, phase3Streams, resetPhase3Mocks } from "./mocks-phase3";
import { phase3bHandlers, phase3bStreams, resetPhase3bMocks } from "./mocks-phase3b";
import { phase4Handlers, resetPhase4Mocks } from "./mocks-phase4";
import type {
  ActivityEntry,
  BackendSettings,
  CleanupCategory,
  CleanupScanResult,
  Crumb,
  DriveInfo,
  SpaceView,
  ViewNode,
} from "./types";

/**
 * Fake backend used outside Tauri (browser dev, Playwright, gallery). No real system access.
 * Numbers are made up but shaped like a real scan.
 */

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** Playwright sets this to skip the artificial delays. */
const fast = () => typeof window !== "undefined" && (window as unknown as { __SMD_FAST_MOCKS__?: boolean }).__SMD_FAST_MOCKS__ === true;
const pause = (ms: number) => (fast() ? Promise.resolve() : sleep(ms));

let nextId = 1000;
let settings: BackendSettings = { exclusions: [], deleteMode: "recycleBin", quarantine: true, quarantineDays: 30, purgeConsent: null, scratchDays: 14, closeToTray: false, guard: true, language: "en", schedule: { frequency: "off", day: 6, hour: 10 } };
let activity: ActivityEntry[] = [];
let lastScanId = 0;

function log(entry: Omit<ActivityEntry, "id" | "timeMs">): ActivityEntry {
  const e = { id: nextId++, timeMs: Date.now(), ...entry };
  activity = [e, ...activity];
  return e;
}

const blank = { bytes: 0, items: 0, failed: 0, score: null, target: null, mode: null, undo: "none" as const };

function cat(id: string, partial: Partial<CleanupCategory> & Pick<CleanupCategory, "group">): CleanupCategory {
  return {
    id,
    safety: "safe",
    admin: false,
    permanent: false,
    defaultSelected: true,
    bytes: 0,
    files: 0,
    items: [],
    excluded: 0,
    running: [],
    unreadable: false,
    moreItems: 0,
    moreBytes: 0,
    ...partial,
  };
}

function items(prefix: string, sizes: number[], dir = false) {
  return sizes.map((bytes, i) => ({
    path: `${prefix}\\item-${i + 1}${dir ? "" : ".tmp"}`,
    name: `item-${i + 1}${dir ? "" : ".tmp"}`,
    bytes,
    files: dir ? 12 : 1,
    modifiedMs: Date.now() - (i + 2) * 86_400_000,
    isDir: dir,
  }));
}

function fakeCleanup(): CleanupScanResult {
  const temp = items("C:\\Users\\you\\AppData\\Local\\Temp", [820 * MB, 310 * MB, 96 * MB, 12 * MB, 3 * MB], true);
  const categories: CleanupCategory[] = [
    cat("user-temp", { group: "system", items: temp }),
    cat("thumbnail-cache", { group: "system", items: items("C:\\Users\\you\\AppData\\Local\\Microsoft\\Windows\\Explorer", [140 * MB, 22 * MB]), running: ["explorer.exe"] }),
    cat("crash-dumps", { group: "system", items: [] }),
    cat("error-reports", { group: "system", items: items("C:\\Users\\you\\AppData\\Local\\Microsoft\\Windows\\WER\\ReportArchive", [4 * MB], true) }),
    cat("edge-cache", {
      group: "browsers",
      running: ["msedge.exe"],
      items: [
        { path: "C:\\Users\\you\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Cache\\Cache_Data", name: "Default\\Cache\\Cache_Data", bytes: 640 * MB, files: 2300, modifiedMs: Date.now(), isDir: true },
        { path: "C:\\Users\\you\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Code Cache", name: "Default\\Code Cache", bytes: 120 * MB, files: 800, modifiedMs: Date.now(), isDir: true },
      ],
    }),
    cat("chrome-cache", { group: "browsers", items: [] }),
    cat("brave-cache", { group: "browsers", items: [] }),
    cat("firefox-cache", { group: "browsers", items: [] }),
    cat("system-temp", { group: "system", admin: true, defaultSelected: false, items: items("C:\\Windows\\Temp", [210 * MB]) }),
    cat("windows-update-cache", { group: "system", admin: true, defaultSelected: false, items: items("C:\\Windows\\SoftwareDistribution\\Download", [1.4 * GB], true) }),
    cat("delivery-optimization", { group: "system", admin: true, defaultSelected: false, unreadable: true }),
    cat("system-error-reports", { group: "system", admin: true, defaultSelected: false, unreadable: true }),
    cat("memory-dumps", { group: "system", admin: true, defaultSelected: false, safety: "advanced" }),
    cat("recycle-bin", { group: "recycleBin", permanent: true, safety: "advanced", defaultSelected: false, bytes: 2.3 * GB, files: 41 }),
  ].map((c) => {
    if (c.items.length && !c.bytes) {
      c.bytes = c.items.reduce((s, i) => s + i.bytes, 0);
      c.files = c.items.reduce((s, i) => s + i.files, 0);
    }
    return c;
  });
  lastScanId = nextId++;
  return {
    scanId: lastScanId,
    categories,
    reclaimableBytes: categories.filter((c) => !c.admin && !c.permanent).reduce((s, c) => s + c.bytes, 0),
    durationMs: 1840,
    adminScanned: false,
  };
}

let lastCleanup: CleanupScanResult | null = null;

async function runSteps(steps: string[], onProgress: (p: { step: string; done: number; total: number }) => void) {
  for (let i = 0; i < steps.length; i++) {
    await pause(120);
    onProgress({ step: steps[i], done: i + 1, total: steps.length });
  }
}

/* ---------- Space Lens mock tree ---------- */
interface MockNode {
  name: string;
  size?: number;
  kind?: ViewNode["kind"];
  children?: MockNode[];
}

const MOCK_TREE: MockNode = {
  name: "C:\\",
  children: [
    {
      name: "Users",
      children: [
        {
          name: "you",
          children: [
            { name: "AppData", children: [{ name: "Local", size: 18 * GB }, { name: "Roaming", size: 4 * GB }] },
            { name: "Downloads", children: [{ name: "installer.iso", kind: "file", size: 5.2 * GB }, { name: "", kind: "others", size: 900 * MB }] },
            { name: "Videos", size: 3.1 * GB },
          ],
        },
      ],
    },
    { name: "Windows", children: [{ name: "WinSxS", size: 9.8 * GB }, { name: "System32", size: 6.4 * GB }, { name: "Installer", size: 3.2 * GB }] },
    { name: "Program Files", children: [{ name: "Big Game", size: 46 * GB }, { name: "Office", size: 4.2 * GB }] },
    { name: "Program Files (x86)", size: 7.6 * GB },
    { name: "pagefile.sys", kind: "file", size: 12 * GB },
    { name: "hiberfil.sys", kind: "file", size: 6.4 * GB },
    { name: "", kind: "others", size: 40 * MB },
  ],
};

interface Flat {
  node: ViewNode;
  parent: number | null;
  childIds: number[];
}

let flat: Map<number, Flat> = new Map();
let spaceScanId = 0;

function buildFlat(): void {
  flat = new Map();
  let id = 0;
  const visit = (n: MockNode, parent: number | null): number => {
    const myId = id++;
    const childIds = (n.children ?? []).map((c) => visit(c, myId));
    const kids = childIds.map((c) => flat.get(c)!.node);
    const size = n.children ? kids.reduce((s, k) => s + k.size, 0) : (n.size ?? 0);
    const kind = n.kind ?? "dir";
    const files = n.children ? kids.reduce((s, k) => s + k.files, 0) : kind === "file" ? 1 : Math.max(1, Math.round(size / (4 * MB)));
    childIds.sort((a, b) => flat.get(b)!.node.size - flat.get(a)!.node.size);
    flat.set(myId, {
      node: { id: myId, name: n.name, kind, size, files, denied: false, childCount: childIds.length, children: [], hiddenCount: 0, hiddenSize: 0 },
      parent,
      childIds,
    });
    return myId;
  };
  visit(MOCK_TREE, null);
}

function mockView(id: number, depth = 2): ViewNode {
  const f = flat.get(id);
  if (!f) throw { code: "notFound", message: `node ${id}`, details: "" };
  return { ...f.node, children: depth > 0 ? f.childIds.map((c) => mockView(c, depth - 1)) : [] };
}

function mockCrumbs(id: number): Crumb[] {
  const out: Crumb[] = [];
  let cur: number | null = id;
  while (cur !== null) {
    const entry: Flat = flat.get(cur)!;
    out.unshift({ id: cur, name: entry.node.name });
    cur = entry.parent;
  }
  return out;
}

function mockSpaceView(id: number): SpaceView {
  const crumbs = mockCrumbs(id);
  const node = flat.get(id)!.node;
  const path = node.kind === "others" ? null : crumbs.length === 1 ? "C:\\" : `C:\\${crumbs.slice(1).map((c) => c.name).join("\\")}`;
  return { scanId: spaceScanId, node: mockView(id), crumbs, path };
}

const MOCK_DRIVES: DriveInfo[] = [
  { letter: "C:", root: "C:\\", label: "Windows", fileSystem: "NTFS", kind: "fixed", totalBytes: 237.5 * GB, freeBytes: 50.6 * GB, isSystem: true },
  { letter: "G:", root: "G:\\", label: "Games", fileSystem: "NTFS", kind: "fixed", totalBytes: 931.5 * GB, freeBytes: 620.8 * GB, isSystem: false },
  { letter: "X:", root: "X:\\", label: "For the Emperor", fileSystem: "NTFS", kind: "fixed", totalBytes: 838.3 * GB, freeBytes: 589.6 * GB, isSystem: false },
];

let spaceCancelled = false;

export const mockHandlers: MockHandlers = {
  app_info: () => ({ version: "0.1.0-mock", windowEffect: "none" }),
  format_bytes: ({ bytes }) => formatBytes(bytes, "en").replace(/\s/u, " "),
  settings_get: () => settings,
  settings_set: ({ settings: s }) => {
    settings = { ...s, exclusions: [...new Set(s.exclusions)].sort() };
    return settings;
  },
  open_log_folder: () => null,
  activity_list: () => activity,
  activity_undo: ({ id }) => {
    const e = activity.find((a) => a.id === id);
    if (!e || e.undo !== "available") throw { code: "stale", message: "cannot undo", details: "" };
    e.undo = "done";
    log({ ...blank, kind: "restore", items: e.items });
    return { restored: e.items, missing: 0, failed: 0 };
  },
  cleanup_run: async ({ scanId, selections }) => {
    if (!lastCleanup || scanId !== lastScanId) throw { code: "stale", message: "scan again", details: "" };
    await pause(600);
    let removedBytes = 0;
    let removedItems = 0;
    let binEmptiedBytes: number | null = null;
    let adminBytes = 0;
    for (const sel of selections) {
      const c = lastCleanup.categories.find((x) => x.id === sel.id);
      if (!c || (c.admin && !lastCleanup.adminScanned)) continue;
      if (c.admin) adminBytes += c.items.filter((i) => !sel.skip.includes(i.path)).reduce((s, i) => s + i.bytes, 0);
      if (c.permanent) {
        binEmptiedBytes = c.bytes;
        log({ ...blank, kind: "emptyRecycleBin", bytes: c.bytes });
        continue;
      }
      for (const i of c.items) {
        if (sel.skip.includes(i.path)) continue;
        removedBytes += i.bytes;
        removedItems += 1;
      }
    }
    // Pretend Explorer keeps one thumbnail database open.
    const failedCount = selections.some((s) => s.id === "thumbnail-cache") ? 1 : 0;
    const e = log({ ...blank, kind: "cleanup", bytes: removedBytes, items: removedItems, failed: failedCount, mode: settings.deleteMode, undo: settings.deleteMode === "recycleBin" && removedItems > 0 ? "available" : "none" });
    lastCleanup = null;
    return {
      mode: settings.deleteMode,
      removedBytes,
      removedItems,
      failedCount,
      failures: failedCount ? [{ path: "C:\\Users\\you\\AppData\\Local\\Microsoft\\Windows\\Explorer\\thumbcache_idx.db", reason: "inUse" as const }] : [],
      binEmptiedBytes,
      undoAvailable: e.undo === "available",
      quarantine: settings.quarantine && settings.deleteMode === "recycleBin" ? "S:\\Quarantine" : null,
      adminBytes,
      activityId: e.id,
    };
  },
  cleanup_admin_scan: async ({ scanId }) => {
    if (!lastCleanup || scanId !== lastScanId) throw { code: "stale", message: "scan again", details: "" };
    await pause(500);
    const cats = lastCleanup.categories.map((c) =>
      c.id === "delivery-optimization"
        ? { ...c, unreadable: false, items: [{ path: "C:\\Windows\\ServiceProfiles\\NetworkService\\AppData\\Local\\Microsoft\\Windows\\DeliveryOptimization\\Cache", name: "Cache", bytes: 3.2 * GB, files: 41, modifiedMs: Date.now(), isDir: true }], bytes: 3.2 * GB, files: 41 }
        : c.admin
          ? { ...c, unreadable: false }
          : c,
    );
    lastCleanup = { ...lastCleanup, categories: cats, adminScanned: true, reclaimableBytes: cats.filter((c) => !c.permanent).reduce((s, c) => s + c.bytes, 0) };
    return lastCleanup;
  },
  drives_list: () => MOCK_DRIVES,
  pick_folder: () => "C:\\",
  space_cancel: () => {
    spaceCancelled = true;
    return null;
  },
  space_view: ({ scanId, nodeId }) => {
    if (scanId !== spaceScanId) throw { code: "stale", message: "scan again", details: "" };
    return mockSpaceView(nodeId);
  },
  space_reveal: () => null,
  ...phase2Handlers(log),
  ...phase3Handlers(log),
  ...phase3bHandlers(log),
  ...phase4Handlers(log),
};

export const mockStreams: MockStreams = {
  cleanup_scan: async (_args, onProgress) => {
    await runSteps(["user-temp", "thumbnail-cache", "edge-cache", "chrome-cache", "recycle-bin"], onProgress);
    lastCleanup = fakeCleanup();
    log({ ...blank, kind: "cleanupScan", bytes: lastCleanup.reclaimableBytes });
    return lastCleanup;
  },
  smart_care_scan: async (_args, onProgress) => {
    await runSteps(["disks", "memory", "user-temp", "edge-cache", "drive"], onProgress);
    lastCleanup = fakeCleanup();
    const result = {
      // One critical area (uptime 34 days): 100 - 18 = 82, "Good".
      assessment: {
        score: 82,
        verdict: "good" as const,
        areas: [
          { area: "diskHealth" as const, status: "good" as const },
          { area: "freeSpace" as const, status: "good" as const },
          { area: "memory" as const, status: "good" as const },
          { area: "maintenance" as const, status: "critical" as const },
        ],
        recommendations: [
          { kind: "restart" as const, uptimeDays: 34 },
          { kind: "cleanJunk" as const, bytes: lastCleanup.reclaimableBytes },
        ],
      },
      systemDrive: MOCK_DRIVES[0],
      memory: { loadPercent: 47, totalBytes: 32 * GB, availableBytes: 17 * GB, commitPercent: 41 },
      memoryMinLoad: 45,
      uptimeSeconds: 34 * 86_400 + 3600,
      disks: [
        { name: "SPCC Solid State Disk", mediaType: "SSD", busType: "SATA", sizeBytes: 256 * 10 ** 9, health: "healthy" as const, operationalStatus: "OK", letters: ["C:"] },
        { name: "ST1000DM003-9YN162", mediaType: "HDD", busType: "SATA", sizeBytes: 10 ** 12, health: "healthy" as const, operationalStatus: "OK", letters: ["G:"] },
      ],
      hasBattery: false,
      cleanup: lastCleanup,
      durationMs: 3200,
      startupEnabled: 5,
      appUpdates: null,
      windowsUpdates: { total: 2, security: 1, titles: [] },
      defender: null,
      activeThreats: 0,
      batteryHealth: null,
      driveLayout: { status: "warning" as const, misplacedBytes: 99 * GB, findings: 4 },
      activityId: 0,
    };
    const e = log({ ...blank, kind: "smartCareScan", score: result.assessment.score, bytes: lastCleanup.reclaimableBytes });
    result.activityId = e.id;
    return result;
  },
  space_scan: async ({ path }, onProgress) => {
    spaceCancelled = false;
    for (let i = 1; i <= 8; i++) {
      await pause(150);
      if (spaceCancelled) {
        return { cancelled: true, rootPath: path, view: null, extensions: [], files: 0, dirs: 0, bytes: 0, denied: 0, durationMs: 0, drive: null };
      }
      onProgress({ files: i * 41_000, dirs: i * 5_200, bytes: i * 15 * GB, denied: i * 3, current: `${path}Windows\\WinSxS\\amd64_part${i}` });
    }
    buildFlat();
    spaceScanId = nextId++;
    const root = flat.get(0)!.node;
    log({ ...blank, kind: "spaceLensScan", bytes: root.size, items: root.files, target: path });
    return {
      cancelled: false,
      rootPath: path,
      view: mockSpaceView(0),
      extensions: [
        { ext: "dll", bytes: 14 * GB, files: 61_000 },
        { ext: "sys", bytes: 18.4 * GB, files: 900 },
        { ext: "iso", bytes: 5.2 * GB, files: 1 },
        { ext: "exe", bytes: 4 * GB, files: 8_000 },
        { ext: "*", bytes: 30 * GB, files: 240_000 },
      ].sort((a, b) => (a.ext === "*" ? 1 : b.ext === "*" ? -1 : b.bytes - a.bytes)),
      files: root.files,
      dirs: 52_000,
      bytes: root.size,
      denied: 24,
      durationMs: 1200,
      drive: MOCK_DRIVES.find((d) => d.root === path) ?? null,
    };
  },
  ...phase2Streams(log),
  ...phase3Streams(log),
  ...phase3bStreams(log),
};

/** Test helper: start every mock session from a clean state. */
export function resetMocks(): void {
  nextId = 1000;
  settings = { exclusions: [], deleteMode: "recycleBin", quarantine: true, quarantineDays: 30, purgeConsent: null, scratchDays: 14, closeToTray: false, guard: true, language: "en", schedule: { frequency: "off", day: 6, hour: 10 } };
  activity = [];
  lastCleanup = null;
  lastScanId = 0;
  resetPhase2Mocks();
  resetPhase3Mocks();
  resetPhase3bMocks();
  resetPhase4Mocks();
}

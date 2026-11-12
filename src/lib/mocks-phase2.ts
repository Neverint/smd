import type { MockHandlers, MockStreams } from "./backend";
import type { ActivityEntry, ClutterView, DownloadsView, InstalledApp, LeftoversView, StartupItem } from "./types";

/** Phase 2 mocks (Performance, Applications, My Clutter). No real system access. */

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const DAY = 86_400_000;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const fast = () => typeof window !== "undefined" && (window as unknown as { __SMD_FAST_MOCKS__?: boolean }).__SMD_FAST_MOCKS__ === true;
const pause = (ms: number) => (fast() ? Promise.resolve() : sleep(ms));

type Log = (entry: Omit<ActivityEntry, "id" | "timeMs">) => ActivityEntry;
const blank = { bytes: 0, items: 0, failed: 0, score: null, target: null, mode: null, undo: "none" as const };

function startupItems(): StartupItem[] {
  const item = (name: string, publisher: string | null, source: StartupItem["source"], enabled: boolean): StartupItem => ({
    id: `${source}:${name}`,
    name,
    command: `"C:\\Program Files\\${name}\\${name}.exe" --minimized`,
    program: `C:\\Program Files\\${name}\\${name}.exe`,
    publisher,
    microsoft: publisher?.includes("Microsoft") ?? false,
    source,
    enabled,
    needsAdmin: source === "machineRun" || source === "machineRun32" || source === "commonFolder",
  });
  return [
    item("Discord", "Discord Inc.", "userRun", true),
    item("OneDrive", "Microsoft Corporation", "userRun", true),
    item("Spotify", "Spotify AB", "userRun", false),
    item("Steam", "Valve Corporation", "userRun", true),
    item("SecurityHealth", "Microsoft Corporation", "machineRun", true),
    item("RtkAudUService", "Realtek Semiconductor", "machineRun", true),
    item("Send to OneNote.lnk", null, "userFolder", true),
  ];
}

let startup = startupItems();

const APPS: InstalledApp[] = [
  { id: "hklm:7-Zip", kind: "desktop", name: "7-Zip 24.08 (x64)", publisher: "Igor Pavlov", version: "24.08", installDate: "2024-10-01", sizeBytes: 5.6 * MB, installLocation: "C:\\Program Files\\7-Zip", microsoft: false, machineWide: true },
  { id: "hkcu:Discord", kind: "desktop", name: "Discord", publisher: "Discord Inc.", version: "1.0.9168", installDate: "2025-02-11", sizeBytes: 380 * MB, installLocation: "C:\\Users\\you\\AppData\\Local\\Discord", microsoft: false, machineWide: false },
  { id: "hklm:Steam", kind: "desktop", name: "Steam", publisher: "Valve Corporation", version: "2.10.91.91", installDate: "2023-06-20", sizeBytes: null, installLocation: "C:\\Program Files (x86)\\Steam", microsoft: false, machineWide: true },
  { id: "hklm:{VLC}", kind: "desktop", name: "VLC media player", publisher: "VideoLAN", version: "3.0.21", installDate: "2024-06-10", sizeBytes: 160 * MB, installLocation: "C:\\Program Files\\VideoLAN\\VLC", microsoft: false, machineWide: true },
  { id: "appx:Microsoft.WindowsCalculator_11.2410.0.0_x64__8wekyb3d8bbwe", kind: "store", name: "Calculator", publisher: "Microsoft Corporation", version: "11.2410.0.0", installDate: null, sizeBytes: null, installLocation: null, microsoft: true, machineWide: false },
  { id: "appx:SpotifyAB.SpotifyMusic_1.250.0.0_x64__zpdnekdrzrea0", kind: "store", name: "Spotify", publisher: "Spotify AB", version: "1.250.0.0", installDate: null, sizeBytes: null, installLocation: null, microsoft: false, machineWide: false },
];

let installed = [...APPS];
let moduleInstalled = false;
let leftoverScan = 0;
let clutterScan = 0;
let downloadsScan = 0;

function clutterView(id: number): ClutterView {
  const f = (path: string, bytes: number, ageDays: number) => ({ path, name: path.split("\\").pop() ?? path, bytes, modifiedMs: Date.now() - ageDays * DAY });
  return {
    scanId: id,
    roots: ["G:\\Desktop", "G:\\Documents", "X:\\Downloads", "G:\\Pictures"],
    durationMs: 4200,
    result: {
      large: [f("X:\\Downloads\\ubuntu-24.04.iso", 5.8 * GB, 200), f("G:\\Videos\\holiday-2019.mp4", 2.1 * GB, 1900), f("G:\\Documents\\backup.zip", 900 * MB, 700)],
      old: [f("G:\\Videos\\holiday-2019.mp4", 2.1 * GB, 1900), f("G:\\Documents\\backup.zip", 900 * MB, 700), f("G:\\Documents\\old-project.psd", 120 * MB, 1200)],
      duplicates: [
        { bytes: 48 * MB, files: [f("G:\\Pictures\\trip\\IMG_0001.mov", 48 * MB, 400), f("X:\\Downloads\\IMG_0001.mov", 48 * MB, 300), f("G:\\Desktop\\IMG_0001 (1).mov", 48 * MB, 290)] },
        { bytes: 3.2 * MB, files: [f("G:\\Documents\\cv.pdf", 3.2 * MB, 100), f("X:\\Downloads\\cv.pdf", 3.2 * MB, 90)] },
      ],
      duplicateBytes: 2 * 48 * MB + 3.2 * MB,
      filesScanned: 48_211,
      bytesScanned: 312 * GB,
      cancelled: false,
    },
  };
}

export function phase2Handlers(log: Log): Omit<MockHandlers, "app_info" | "format_bytes" | "settings_get" | "settings_set" | "open_log_folder" | "activity_list" | "activity_undo" | "cleanup_run" | "drives_list" | "pick_folder" | "space_cancel" | "space_view" | "space_reveal"> {
  return {
    startup_list: () => startup,
    startup_set: ({ id, enabled }) => {
      const item = startup.find((s) => s.id === id);
      if (!item) throw { code: "notFound", message: id, details: "" };
      if (item.needsAdmin) throw { code: "accessDenied", message: "needs admin", details: "" };
      startup = startup.map((s) => (s.id === id ? { ...s, enabled } : s));
      log({ ...blank, kind: "startupChange", target: item.name, enabled, items: 1, undo: "available" });
      return startup;
    },
    processes_snapshot: () => {
      const jitter = () => Math.round(Math.random() * 40) / 10;
      return {
        cpuPercent: 12 + jitter(),
        processCount: 214,
        byCpu: [
          { name: "msedge.exe", processes: 24, cpuPercent: 4.1 + jitter(), memoryBytes: 1.9 * GB },
          { name: "Discord.exe", processes: 6, cpuPercent: 1.2, memoryBytes: 610 * MB },
          { name: "smd.exe", processes: 1, cpuPercent: 0.4, memoryBytes: 64 * MB },
        ],
        byMemory: [
          { name: "msedge.exe", processes: 24, cpuPercent: 4.1, memoryBytes: 1.9 * GB },
          { name: "Claude.exe", processes: 9, cpuPercent: 0.8, memoryBytes: 1.1 * GB },
          { name: "Discord.exe", processes: 6, cpuPercent: 1.2, memoryBytes: 610 * MB },
        ],
      };
    },
    processes_stop: () => null,
    background_list: async () => {
      await pause(300);
      return {
        tasks: [
          { name: "GoogleUpdateTaskMachineCore", path: "\\", state: "Ready", author: "Google LLC", program: "C:\\Program Files (x86)\\Google\\Update\\GoogleUpdate.exe", publisher: "Google LLC" },
          { name: "NvTmRep_CrashReport1", path: "\\NVIDIA\\", state: "Disabled", author: "NVIDIA", program: null, publisher: "NVIDIA Corporation" },
        ],
        services: [
          { name: "Steam Client Service", displayName: "Steam Client Service", state: "Stopped", startMode: "Manual", program: "C:\\Program Files (x86)\\Common Files\\Steam\\steamservice.exe", publisher: "Valve Corporation" },
          { name: "NVDisplay.ContainerLocalSystem", displayName: "NVIDIA Display Container LS", state: "Running", startMode: "Auto", program: null, publisher: "NVIDIA Corporation" },
        ],
        boot: { lastBootMs: Date.now() - 18 * DAY, bootDurationMs: null, mainPathMs: null },
      };
    },
    open_system_tool: () => null,
    apps_list: () => installed,
    apps_leftovers: ({ id }) => {
      const app = APPS.find((a) => a.id === id);
      if (!app) throw { code: "stale", message: id, details: "" };
      leftoverScan = Date.now();
      const view: LeftoversView = {
        scanId: leftoverScan,
        appName: app.name,
        items: [
          { path: `${app.installLocation ?? "C:\\Program Files\\App"}`, kind: "folder", bytes: 12 * MB, confidence: "high", needsAdmin: app.machineWide, selected: !app.machineWide },
          { path: `C:\\Users\\you\\AppData\\Roaming\\${app.name.split(" ")[0]}`, kind: "folder", bytes: 3 * MB, confidence: "medium", needsAdmin: false, selected: false },
          { path: `HKCU\\Software\\${app.name.split(" ")[0]}`, kind: "registryKey", bytes: 0, confidence: "medium", needsAdmin: false, selected: false },
        ],
      };
      return view;
    },
    apps_remove_leftovers: ({ scanId, paths }) => {
      if (scanId !== leftoverScan) throw { code: "stale", message: "scan again", details: "" };
      leftoverScan = 0;
      log({ ...blank, kind: "leftoversRemoved", items: paths.length, bytes: 15 * MB, undo: "available" });
      return { removedBytes: 15 * MB, removedItems: paths.length, failures: [], undoAvailable: paths.length > 0 };
    },
    updates_status: () => ({ moduleInstalled }),
    updates_setup: async () => {
      await pause(800);
      moduleInstalled = true;
      log({ ...blank, kind: "updatesSetup", items: 1 });
      return { moduleInstalled };
    },
    updates_list: async () => {
      await pause(500);
      return [
        { id: "VideoLAN.VLC", name: "VLC media player", installed: "3.0.21", available: "3.0.22", source: "winget" },
        { id: "7zip.7zip", name: "7-Zip", installed: "24.08", available: "24.09", source: "winget" },
      ];
    },
    windows_updates: () => ({ total: 2, security: 1, titles: ["2026-11 Cumulative Update for Windows 11 (KB5099999)", "Windows Malicious Software Removal Tool"] }),
    clutter_cancel: () => null,
    clutter_downloads: (): DownloadsView => {
      downloadsScan = Date.now() + 1;
      const item = (name: string, bytes: number, ageDays: number, isDir = false) => ({ path: `X:\\Downloads\\${name}`, name, bytes, files: isDir ? 40 : 1, modifiedMs: Date.now() - ageDays * DAY, isDir });
      return {
        scanId: downloadsScan,
        path: "X:\\Downloads",
        items: [item("ubuntu-24.04.iso", 5.8 * GB, 200), item("GameSetup_v2", 1.2 * GB, 420, true), item("invoice-0425.pdf", 220_000, 30), item("IMG_0001.mov", 48 * MB, 300)],
      };
    },
    clutter_remove: ({ scanId, paths }) => {
      if (scanId !== clutterScan && scanId !== downloadsScan) throw { code: "stale", message: "scan again", details: "" };
      log({ ...blank, kind: "clutterRemoved", items: paths.length, bytes: paths.length * 50 * MB, target: "scan", undo: "available" });
      return { removedBytes: paths.length * 50 * MB, removedItems: paths.length, failures: [], undoAvailable: true };
    },
  };
}

export function phase2Streams(log: Log): Pick<MockStreams, "apps_uninstall" | "updates_apply" | "clutter_scan"> {
  return {
    apps_uninstall: async ({ ids }, onProgress) => {
      const out = [];
      for (let i = 0; i < ids.length; i++) {
        const app = installed.find((a) => a.id === ids[i]);
        if (!app) throw { code: "stale", message: ids[i], details: "" };
        onProgress({ index: i + 1, total: ids.length, name: app.name });
        await pause(500);
        installed = installed.filter((a) => a.id !== app.id);
        log({ ...blank, kind: "uninstall", target: app.name, items: 1 });
        out.push({ id: app.id, name: app.name, result: { exitCode: 0, stillInstalled: false }, error: null });
      }
      return out;
    },
    updates_apply: async ({ ids }, onProgress) => {
      const out = [];
      for (let i = 0; i < ids.length; i++) {
        onProgress({ index: i + 1, total: ids.length, id: ids[i] });
        await pause(500);
        log({ ...blank, kind: "appUpdate", target: ids[i], items: 1 });
        out.push({ id: ids[i], result: { status: "Ok", rebootRequired: false, installerErrorCode: null }, error: null });
      }
      return out;
    },
    clutter_scan: async (_args, onProgress) => {
      for (let i = 1; i <= 6; i++) {
        await pause(150);
        onProgress({ stage: i < 4 ? "walk" : "compare", files: i * 8000, bytes: i * 52 * GB, hashed: i < 4 ? 0 : (i - 3) * 40 * MB, toHash: 150 * MB });
      }
      clutterScan = Date.now();
      return clutterView(clutterScan);
    },
  };
}

export function resetPhase2Mocks(): void {
  startup = startupItems();
  installed = [...APPS];
  moduleInstalled = false;
  leftoverScan = 0;
  clutterScan = 0;
  downloadsScan = 0;
}

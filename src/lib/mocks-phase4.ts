import toolboxJson from "../../rules/toolbox.json";
import tweaksJson from "../../rules/tweaks.json";
import type { MockHandlers } from "./backend";
import type { ActivityEntry, ApplyReport, DiskReliability, ScheduleView, Tool, ToolboxView, Tweak, TweakState, TweaksView } from "./types";

/** Phase 4 mocks (Tweaks, Toolbox, tray, schedule, admin helper). No real system access. */

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const fast = () => typeof window !== "undefined" && (window as unknown as { __SMD_FAST_MOCKS__?: boolean }).__SMD_FAST_MOCKS__ === true;
const pause = (ms: number) => (fast() ? Promise.resolve() : sleep(ms));

type Log = (entry: Omit<ActivityEntry, "id" | "timeMs">) => ActivityEntry;
const blank = { bytes: 0, items: 0, failed: 0, score: null, target: null, mode: null, undo: "none" as const };

type RawTweak = Omit<Tweak, "state" | "needsAdmin" | "appliedMs" | "reinstall" | "restartExplorer" | "restartWindows" | "appx"> & {
  restartExplorer?: boolean;
  restartWindows?: boolean;
  appx?: string;
  registry?: { hive: string }[];
  services?: unknown[];
};

const INSTALLED_APPS = new Set(["Microsoft.BingNews", "Microsoft.BingWeather", "Microsoft.GetHelp", "Clipchamp.Clipchamp", "Microsoft.Todos", "MSTeams", "Microsoft.YourPhone", "Microsoft.MicrosoftSolitaireCollection"]);
const ALREADY = new Set(["show-file-extensions", "taskbar-end-task"]);

let tweakState = new Map<string, { state: TweakState; appliedMs: number | null; reinstall: boolean }>();
let adminSession = false;
let restorePointMade = 0;
let installed = new Map<string, string | null>([["crystaldiskinfo", "9.9.4"], ["powertoys", "0.102.3101"]]);
let schedule: ScheduleView = { schedule: { frequency: "off", day: 6, hour: 10 }, registered: false };
let recycleBin = 2.3 * GB;

function initialState(t: RawTweak): TweakState {
  if (t.appx) return INSTALLED_APPS.has(t.appx) ? "notApplied" : "notInstalled";
  if (ALREADY.has(t.id)) return "applied";
  return t.id === "start-suggestions" ? "partial" : "notApplied";
}

function tweaks(): Tweak[] {
  return (tweaksJson.tweaks as unknown as RawTweak[]).map((t) => {
    const s = tweakState.get(t.id) ?? { state: initialState(t), appliedMs: null, reinstall: false };
    return {
      ...t,
      restartExplorer: t.restartExplorer ?? false,
      restartWindows: t.restartWindows ?? false,
      appx: t.appx ?? null,
      needsAdmin: (t.services?.length ?? 0) > 0 || (t.registry ?? []).some((r) => r.hive === "hklm"),
      ...s,
    };
  });
}

const view = (): TweaksView => ({ tweaks: tweaks(), adminSession });

function toolbox(): ToolboxView {
  const tools: Tool[] = (toolboxJson.tools as unknown as Omit<Tool, "installed" | "version" | "canOpen">[]).map((t) => ({
    ...t,
    installed: installed.has(t.id),
    version: installed.get(t.id) ?? null,
    canOpen: installed.has(t.id) && t.id !== "pawnio",
  }));
  return { tools, checkedMs: Date.now(), wingetError: null };
}

export function resetPhase4Mocks(): void {
  tweakState = new Map();
  adminSession = false;
  restorePointMade = 0;
  installed = new Map([["crystaldiskinfo", "9.9.4"], ["powertoys", "0.102.3101"]]);
  schedule = { schedule: { frequency: "off", day: 6, hour: 10 }, registered: false };
  recycleBin = 2.3 * GB;
}

let tick = 0;

export function phase4Handlers(log: Log): Pick<
  MockHandlers,
  | "tweaks_list"
  | "tweaks_apply"
  | "tweak_undo"
  | "restart_explorer"
  | "admin_session_end"
  | "toolbox_list"
  | "toolbox_install"
  | "toolbox_open"
  | "disk_reliability"
  | "sensors_admin_poll"
  | "sensors_admin_stop"
  | "tray_info"
  | "tray_sample"
  | "tray_action"
  | "tray_hide"
  | "schedule_get"
  | "schedule_set"
> {
  return {
    tweaks_list: async () => {
      await pause(300);
      return view();
    },
    tweaks_apply: async ({ ids, withoutRestorePoint }): Promise<ApplyReport> => {
      await pause(700);
      const all = tweaks();
      const chosen = all.filter((t) => ids.includes(t.id));
      const needsAdmin = chosen.some((t) => t.needsAdmin);
      let restorePoint: ApplyReport["restorePoint"] = null;
      if (needsAdmin) {
        adminSession = true;
        restorePoint = restorePointMade ? { status: "recent", timeMs: restorePointMade } : { status: "created", timeMs: Date.now() };
        if (!restorePointMade) {
          restorePointMade = Date.now();
          log({ ...blank, kind: "restorePoint", items: 1 });
        }
      }
      void withoutRestorePoint;
      for (const t of chosen) {
        tweakState.set(t.id, { state: t.appx ? "notInstalled" : "applied", appliedMs: Date.now(), reinstall: !!t.appx });
      }
      const e = log({ ...blank, kind: "tweaksApplied", items: chosen.length, undo: "available" });
      return {
        restorePoint,
        needsConfirm: false,
        applied: chosen.map((t) => t.id),
        failed: [],
        restartExplorer: chosen.some((t) => t.restartExplorer),
        restartWindows: chosen.some((t) => t.restartWindows),
        activityId: e.id,
        view: view(),
      };
    },
    tweak_undo: async ({ id }) => {
      await pause(300);
      const t = tweaks().find((x) => x.id === id);
      if (t && !t.appx) {
        tweakState.set(id, { state: "notApplied", appliedMs: null, reinstall: false });
        log({ ...blank, kind: "restore", items: 1, target: id });
      }
      return view();
    },
    restart_explorer: () => null,
    admin_session_end: () => {
      adminSession = false;
      return null;
    },
    toolbox_list: async ({ refreshNow }) => {
      await pause(refreshNow ? 900 : 300);
      return toolbox();
    },
    toolbox_install: async ({ id, update }) => {
      await pause(1200);
      const tool = toolbox().tools.find((t) => t.id === id);
      installed.set(id, update ? (installed.get(id) ?? null) : "1.0");
      log({ ...blank, kind: update ? "toolUpdate" : "toolInstall", items: 1, target: tool?.name ?? id });
      return { result: { ok: true, code: 0, output: update ? "No available upgrade found." : "Successfully installed" }, view: toolbox() };
    },
    toolbox_open: () => null,
    disk_reliability: async (): Promise<DiskReliability[]> => {
      await pause(400);
      adminSession = true;
      return [
        { serial: "S6B0NL0W123456", name: "Samsung SSD 990 PRO with Heatsink 1TB", temperature: 38, temperatureMax: 71, wear: 2, powerOnHours: 4120, readErrors: 0, writeErrors: 0 },
        { serial: "Z1D5ABCD", name: "ST1000DM003-9YN162", temperature: 34, temperatureMax: 52, wear: null, powerOnHours: 38_900, readErrors: 0, writeErrors: 0 },
      ];
    },
    sensors_admin_poll: () => {
      adminSession = true;
      tick++;
      const w = (tick % 5) - 2;
      return {
        running: true,
        exitCode: null,
        snapshot: {
          elevated: true,
          error: null,
          hardware: [
            {
              name: "AMD Ryzen 5 5600X",
              kind: "Cpu",
              sensors: [
                { name: "CPU Total", kind: "Load", value: 12 + w },
                { name: "Core (Tctl/Tdie)", kind: "Temperature", value: 46 + w },
                { name: "Package", kind: "Power", value: 31 + w },
              ],
            },
            { name: "Nuvoton NCT6797D", kind: "SuperIO", sensors: [{ name: "Fan #2", kind: "Fan", value: 820 + w * 10 }] },
          ],
        },
      };
    },
    sensors_admin_stop: () => null,
    tray_info: () => ({
      systemDrive: "C:",
      systemFree: 52.3 * GB,
      systemTotal: 237.5 * GB,
      recycleBinBytes: recycleBin,
      lastScore: 92,
      lastScoreMs: Date.now() - 3 * 3600_000,
      cpuTempC: null,
      busy: false,
    }),
    tray_sample: () => {
      tick++;
      return {
        cpuPercent: 8 + ((tick * 7) % 23),
        memoryPercent: 61 + (tick % 4),
        diskBytesPerSec: ((tick * 13) % 40) * MB,
        netBytesPerSec: ((tick * 5) % 12) * 120 * 1024,
      };
    },
    tray_action: ({ action }) => {
      if (action === "emptyRecycleBin") {
        log({ ...blank, kind: "emptyRecycleBin", bytes: recycleBin });
        recycleBin = 0;
      }
      return null;
    },
    tray_hide: () => null,
    schedule_get: () => schedule,
    schedule_set: async ({ schedule: s }) => {
      await pause(300);
      schedule = { schedule: s, registered: s.frequency !== "off" };
      return schedule;
    },
  };
}

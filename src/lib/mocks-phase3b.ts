import type { MockHandlers, MockStreams } from "./backend";
import type { ActivityEntry, AuditView, Finding, PlanView, Policy, QuarantineBatch } from "./types";

/** Phase 3b mocks (Drive Organizer, quarantine, scratch). No real system access. */

const GB = 1024 ** 3;
const MB = 1024 ** 2;
const DAY = 86_400_000;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const fast = () => typeof window !== "undefined" && (window as unknown as { __SMD_FAST_MOCKS__?: boolean }).__SMD_FAST_MOCKS__ === true;
const pause = (ms: number) => (fast() ? Promise.resolve() : sleep(ms));

type Log = (entry: Omit<ActivityEntry, "id" | "timeMs">) => ActivityEntry;
const blank = { bytes: 0, items: 0, failed: 0, score: null, target: null, mode: null, undo: "none" as const };

const defaultPolicy = (): Policy => ({
  version: 1,
  gameThresholdGb: 50,
  drives: [
    { letter: "C:", role: "system" },
    { letter: "X:", role: "heavyGames" },
    { letter: "G:", role: "lightGames" },
    { letter: "S:", role: "disposable" },
  ],
});

let policy = defaultPolicy();
let moved = new Set<string>();
let purgeConsent: boolean | null = null;
let planId = 0;
let batches: QuarantineBatch[] = [];

function findings(): Finding[] {
  const all: Finding[] = [
    { id: "g1", name: "Elden Ring", path: "G:\\Games\\Steam\\steamapps\\common\\ELDEN RING", drive: "G:", bytes: 58 * GB, severity: "problem", reason: "bigGameOnLightDrive", targetDrive: "X:", method: { kind: "launcherMove", launcher: "steam" } },
    { id: "g2", name: "Counter-Strike 2", path: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Counter-Strike Global Offensive", drive: "C:", bytes: 33 * GB, severity: "problem", reason: "gameOnSystemDrive", targetDrive: "G:", method: { kind: "launcherMove", launcher: "steam" } },
    { id: "p1", name: "holiday-2019.mp4", path: "C:\\Users\\you\\Stuff\\holiday-2019.mp4", drive: "C:", bytes: 2.1 * GB, severity: "warning", reason: "personalFilesOnSystemDrive", targetDrive: "G:", method: { kind: "copyVerify" } },
    { id: "c1", name: "Hugging Face models", path: "C:\\Users\\you\\.cache\\huggingface", drive: "C:", bytes: 6.4 * GB, severity: "warning", reason: "cacheOnSystemDrive", targetDrive: "X:", method: { kind: "relocateCache", envVar: "HF_HOME" } },
    { id: "i1", name: "Football Manager 2024", path: "X:\\Games\\FootballManager24", drive: "X:", bytes: 5.3 * GB, severity: "info", reason: "smallGameOnHeavyDrive", targetDrive: "G:", method: { kind: "reinstall", launcher: "epic" } },
  ];
  return all.filter((f) => !moved.has(f.id));
}

function audit(): AuditView {
  const f = findings();
  const misplaced = f.filter((x) => x.severity !== "info").reduce((s, x) => s + x.bytes, 0);
  return {
    auditId: Date.now(),
    policy,
    drives: [
      { letter: "C:", label: "", role: policy.drives.find((d) => d.letter === "C:")?.role ?? "unassigned", totalBytes: 237 * GB, freeBytes: 30.8 * GB, diskName: "SPCC Solid State Disk", mediaType: "SSD", busType: "SATA", serial: "GMAJ220443679", health: "healthy" },
      { letter: "X:", label: "For the Emperor", role: policy.drives.find((d) => d.letter === "X:")?.role ?? "unassigned", totalBytes: 838 * GB, freeBytes: 589 * GB, diskName: "Samsung SSD 990 PRO with Heatsink 1TB", mediaType: "SSD", busType: "NVMe", serial: "0025_384A", health: "healthy" },
      { letter: "G:", label: "Games", role: policy.drives.find((d) => d.letter === "G:")?.role ?? "unassigned", totalBytes: 931 * GB, freeBytes: 620 * GB, diskName: "ST1000DM003-9YN162", mediaType: "HDD", busType: "SATA", serial: "Z1D1VQTX", health: "healthy" },
      { letter: "S:", label: "Storage (slow)", role: policy.drives.find((d) => d.letter === "S:")?.role ?? "unassigned", totalBytes: 931 * GB, freeBytes: 931 * GB, diskName: "WDC WD10SPZX-00Z10T0", mediaType: "HDD", busType: "SATA", serial: "WD-WXN1E583ZSRC", health: "healthy" },
    ],
    notices: [],
    findings: f,
    status: misplaced > 10 * GB ? "warning" : "good",
    misplacedBytes: misplaced,
    games: 24,
    unfinished: [],
    quarantine: { drive: "S:", active: true, bytes: batches.reduce((s, b) => s + b.bytes, 0), batches: batches.length, oldBytes: batches.filter((b) => b.createdMs < Date.now() - 30 * DAY).reduce((s, b) => s + b.bytes, 0), purgeConsent, days: 30 },
    durationMs: 3400,
  };
}

function plan(ids: string[]): PlanView {
  planId = Date.now();
  const steps = findings()
    .filter((f) => ids.includes(f.id))
    .map((f) => {
      const runs = f.method.kind === "copyVerify" || f.method.kind === "relocateCache";
      const blocked = f.id === "c1";
      return {
        finding: f,
        target: runs ? (f.method.kind === "relocateCache" ? "X:\\Caches\\huggingface" : `G:\\From C\\${f.path.slice(3)}`) : null,
        estSeconds: Math.round((f.bytes * 2) / (120 * MB)),
        runsInApp: runs,
        ready: !blocked,
        checks: [
          { kind: "targetRole" as const, ok: true, detail: "" },
          { kind: "freeSpace" as const, ok: true, detail: String(f.bytes * 1.1) },
          { kind: "sourceHealth" as const, ok: true, detail: "" },
          { kind: "targetHealth" as const, ok: true, detail: "" },
          { kind: "targetFree" as const, ok: true, detail: "" },
          { kind: "filesInUse" as const, ok: true, detail: "" },
          { kind: "programsClosed" as const, ok: !blocked, detail: blocked ? "python.exe" : "" },
          { kind: "pluggedIn" as const, ok: true, detail: "" },
        ],
      };
    });
  const ready = steps.filter((s) => s.ready && s.runsInApp);
  return { planId, steps, readyBytes: ready.reduce((s, x) => s + x.finding.bytes, 0), estSeconds: ready.reduce((s, x) => s + x.estSeconds, 0) };
}

export function phase3bHandlers(
  log: Log,
): Pick<MockHandlers, "organizer_audit" | "organizer_policy_set" | "organizer_plan" | "organizer_cancel" | "quarantine_list" | "quarantine_restore" | "quarantine_purge_consent" | "clutter_scratch"> {
  return {
    organizer_audit: async () => {
      await pause(400);
      return audit();
    },
    organizer_policy_set: ({ policy: p }) => {
      policy = p;
      return audit();
    },
    organizer_plan: ({ ids }) => plan(ids),
    organizer_cancel: () => null,
    quarantine_list: () => batches,
    quarantine_restore: ({ manifest }) => {
      const b = batches.find((x) => x.manifest === manifest);
      batches = batches.filter((x) => x.manifest !== manifest);
      return { restored: b?.items ?? 0, conflicts: 0, failed: 0 };
    },
    quarantine_purge_consent: ({ allow }) => {
      purgeConsent = allow;
      if (!allow) return 0;
      const old = batches.filter((b) => b.createdMs < Date.now() - 30 * DAY);
      batches = batches.filter((b) => !old.includes(b));
      const freed = old.reduce((s, b) => s + b.bytes, 0);
      if (freed) log({ ...blank, kind: "quarantinePurge", bytes: freed });
      return freed;
    },
    clutter_scratch: () => ({
      scanId: Date.now() + 7,
      path: "S:\\Scratch",
      items: [{ path: "S:\\Scratch\\unzipped-mod", name: "unzipped-mod", bytes: 820 * MB, files: 312, modifiedMs: Date.now() - 40 * DAY, isDir: true }],
    }),
  };
}

export function phase3bStreams(log: Log): Pick<MockStreams, "organizer_run" | "organizer_resume"> {
  return {
    organizer_run: async ({ planId: id, findingIds }, onProgress) => {
      if (id !== planId) throw { code: "stale", message: "plan again", details: "" };
      const fs = findings().filter((f) => findingIds.includes(f.id));
      const total = fs.reduce((s, f) => s + f.bytes, 0);
      for (let i = 1; i <= 5; i++) {
        await pause(150);
        onProgress({ step: 0, steps: fs.length, bytesDone: (total * i) / 5, bytesTotal: total });
      }
      fs.forEach((f) => moved.add(f.id));
      log({ ...blank, kind: "migration", bytes: total, items: fs.length, undo: "available" });
      return {
        journal: { id: 1, createdMs: Date.now(), steps: fs.length, done: fs.length, failed: 0, bytes: total },
        steps: fs.map((f) => ({ findingId: f.id, name: f.name, source: f.path, target: `G:\\From C\\${f.path.slice(3)}`, bytes: f.bytes, state: "done" as const, error: null, skippedLinks: 0 })),
        cancelled: false,
        undoAvailable: true,
      };
    },
    organizer_resume: async () => ({ journal: { id: 1, createdMs: Date.now(), steps: 0, done: 0, failed: 0, bytes: 0 }, steps: [], cancelled: false, undoAvailable: false }),
  };
}

/** Test helper: an old quarantine batch, so the purge question shows. */
export function seedQuarantine(): void {
  batches = [{ id: 1, manifest: "S:\\Quarantine\\2026-09-30\\1\\manifest.json", createdMs: Date.now() - 43 * DAY, bytes: 1.2 * GB, items: 14, sample: ["C:\\Users\\you\\AppData\\Local\\Temp\\setup-tmp"] }];
}

export function resetPhase3bMocks(): void {
  policy = defaultPolicy();
  moved = new Set();
  purgeConsent = null;
  planId = 0;
  seedQuarantine();
}

seedQuarantine();

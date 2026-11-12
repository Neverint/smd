import type { MockHandlers, MockStreams } from "./backend";
import type { ActivityEntry, CapabilityState, HardwareView, ProtectionView, SensorsView } from "./types";

/** Phase 3 mocks (Protection, Hardware Health). No real system access. */

const GB = 1024 ** 3;
const DAY = 86_400_000;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const fast = () => typeof window !== "undefined" && (window as unknown as { __SMD_FAST_MOCKS__?: boolean }).__SMD_FAST_MOCKS__ === true;
const pause = (ms: number) => (fast() ? Promise.resolve() : sleep(ms));

type Log = (entry: Omit<ActivityEntry, "id" | "timeMs">) => ActivityEntry;
const blank = { bytes: 0, items: 0, failed: 0, score: null, target: null, mode: null, undo: "none" as const };

function initialPermissions(): CapabilityState[] {
  const app = (capability: CapabilityState["capability"], id: string, packaged: boolean, allowed: boolean, daysAgo: number | null, inUse = false) => ({
    capability,
    id,
    app: packaged ? id.split("_")[0] : id.replaceAll("#", "\\"),
    packaged,
    allowed,
    lastUsedMs: daysAgo === null ? null : Date.now() - daysAgo * DAY,
    inUse,
  });
  return [
    { capability: "webcam", allowed: true, apps: [app("webcam", "Microsoft.WindowsCamera_8wekyb3d8bbwe", true, true, 12), app("webcam", "MSTeams_8wekyb3d8bbwe", true, true, 2)] },
    {
      capability: "microphone",
      allowed: true,
      apps: [app("microphone", "C:#Users#you#AppData#Local#Discord#app-1.0.9168#Discord.exe", false, true, 0, true), app("microphone", "Microsoft.WindowsSoundRecorder_8wekyb3d8bbwe", true, true, null)],
    },
    { capability: "location", allowed: true, apps: [app("location", "Microsoft.WindowsMaps_8wekyb3d8bbwe", true, true, 40), app("location", "Microsoft.BingWeather_8wekyb3d8bbwe", true, false, 90)] },
  ];
}

let permissions = initialPermissions();
let lastScanMs = Date.now() - 2 * DAY;
let sensorTicks = 0;

function protectionView(): ProtectionView {
  return {
    defender: {
      serviceEnabled: true,
      antivirusEnabled: true,
      realTime: true,
      tamperProtected: true,
      signaturesUpdatedMs: Date.now() - 5 * 3_600_000,
      signatureAgeDays: 0,
      quickScanEndMs: lastScanMs,
      fullScanEndMs: Date.now() - 178 * DAY,
      runningMode: "Normal",
    },
    threats: [{ name: "PUA:Win32/Presenoker", severity: "Low", detectedMs: Date.now() - 30 * DAY, status: "Quarantined", active: false, resources: ["file:_C:\\Users\\you\\Downloads\\free-tool-setup.exe"] }],
    firewall: [
      { name: "Domain", enabled: true },
      { name: "Private", enabled: true },
      { name: "Public", enabled: true },
    ],
    permissions,
  };
}

export function phase3Handlers(log: Log): Pick<MockHandlers, "protection_status" | "protection_cancel_scan" | "protection_update_signatures" | "permission_set" | "hardware_info" | "sensors_poll" | "sensors_stop"> {
  return {
    protection_status: async () => {
      await pause(300);
      return protectionView();
    },
    protection_cancel_scan: () => null,
    protection_update_signatures: async () => {
      await pause(600);
      log({ ...blank, kind: "signatureUpdate", items: 1 });
      return protectionView().defender;
    },
    permission_set: ({ capability, id, allowed }) => {
      const entry = permissions.find((c) => c.capability === capability)?.apps.find((a) => a.id === id);
      if (!entry || !entry.packaged) throw { code: "accessDenied", message: "only Store apps", details: "" };
      permissions = permissions.map((c) => (c.capability !== capability ? c : { ...c, apps: c.apps.map((a) => (a.id === id ? { ...a, allowed } : a)) }));
      log({ ...blank, kind: "permissionChange", target: `${capability}|${entry.app}`, enabled: allowed, items: 1, undo: "available" });
      return permissions;
    },
    hardware_info: async (): Promise<HardwareView> => {
      await pause(300);
      return {
        system: {
          cpu: { name: "AMD Ryzen 5 5600X 6-Core Processor", cores: 6, threads: 12, maxMhz: 3701 },
          gpus: [{ name: "AMD Radeon RX 6700 XT", driverVersion: "32.0.21025.1024" }],
          modules: [
            { slot: "DIMM 1", bank: "P0 CHANNEL A", capacityBytes: 8 * GB, speedMts: 3200, partNumber: "CMK16GX4M2B3200C16", manufacturer: "Corsair", kind: "DDR4" },
            { slot: "DIMM 1", bank: "P0 CHANNEL B", capacityBytes: 8 * GB, speedMts: 3200, partNumber: "CMK16GX4M2B3200C16", manufacturer: "Corsair", kind: "DDR4" },
          ],
          slots: 4,
        },
        disks: [
          { name: "Samsung SSD 990 PRO with Heatsink 1TB", mediaType: "SSD", busType: "NVMe", sizeBytes: 10 ** 12, health: "healthy", operationalStatus: "OK", letters: ["X:"] },
          { name: "ST1000DM003-9YN162", mediaType: "HDD", busType: "SATA", sizeBytes: 10 ** 12, health: "healthy", operationalStatus: "OK", letters: ["G:"] },
        ],
        memory: { loadPercent: 64, totalBytes: 16 * GB, availableBytes: 5.8 * GB, commitPercent: 51 },
        batteries: [],
        hasBattery: false,
        pawnio: false,
      };
    },
    sensors_poll: (): SensorsView => {
      sensorTicks++;
      const wobble = (sensorTicks % 5) - 2;
      return {
        running: true,
        exitCode: null,
        snapshot: {
          elevated: false,
          error: null,
          hardware: [
            { name: "AMD Ryzen 5 5600X", kind: "Cpu", sensors: [{ name: "CPU Total", kind: "Load", value: 12 + wobble }] },
            {
              name: "AMD Radeon RX 6700 XT",
              kind: "GpuAmd",
              sensors: [
                { name: "GPU Core", kind: "Temperature", value: 34 + wobble },
                { name: "GPU Hot Spot", kind: "Temperature", value: 38 + wobble },
                { name: "GPU Fan", kind: "Fan", value: 0 },
                { name: "GPU Package", kind: "Power", value: 9 },
                { name: "D3D 3D", kind: "Load", value: 3 },
              ],
            },
          ],
        },
      };
    },
    sensors_stop: () => null,
  };
}

export function phase3Streams(log: Log): Pick<MockStreams, "protection_scan"> {
  return {
    protection_scan: async ({ kind }, onProgress) => {
      for (let i = 1; i <= 5; i++) {
        await pause(200);
        onProgress({ elapsedMs: i * 1000 });
      }
      lastScanMs = Date.now();
      log({ ...blank, kind: "defenderScan", target: kind });
      return { cancelled: false, threatsFound: false, exitCode: 0, durationMs: 5000, threats: protectionView().threats };
    },
  };
}

export function resetPhase3Mocks(): void {
  permissions = initialPermissions();
  lastScanMs = Date.now() - 2 * DAY;
  sensorTicks = 0;
}

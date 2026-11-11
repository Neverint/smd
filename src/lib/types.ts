/** Types returned by the Rust backend. Keep in sync with the serde structs (camelCase). */

export type WindowEffect = "mica" | "acrylic" | "none";

export interface AppInfo {
  version: string;
  windowEffect: WindowEffect;
}

/* ---------- Settings ---------- */
export type DeleteMode = "recycleBin" | "permanent";

export interface BackendSettings {
  exclusions: string[];
  deleteMode: DeleteMode;
}

/* ---------- Cleanup ---------- */
export type CleanupGroup = "system" | "browsers" | "recycleBin";
export type Safety = "safe" | "advanced";

export interface CleanupItem {
  path: string;
  name: string;
  bytes: number;
  files: number;
  modifiedMs: number | null;
  isDir: boolean;
}

export interface CleanupCategory {
  id: string;
  group: CleanupGroup;
  safety: Safety;
  admin: boolean;
  permanent: boolean;
  defaultSelected: boolean;
  bytes: number;
  files: number;
  items: CleanupItem[];
  excluded: number;
  running: string[];
  unreadable: boolean;
  moreItems: number;
  moreBytes: number;
}

export interface CleanupScanResult {
  scanId: number;
  categories: CleanupCategory[];
  reclaimableBytes: number;
  durationMs: number;
}

export interface ScanProgress {
  step: string;
  done: number;
  total: number;
}

export interface CleanupSelection {
  id: string;
  skip: string[];
}

export type FailReason = "inUse" | "gone" | "refused";

export interface CleanupReport {
  mode: DeleteMode;
  removedBytes: number;
  removedItems: number;
  failedCount: number;
  failures: { path: string; reason: FailReason }[];
  binEmptiedBytes: number | null;
  undoAvailable: boolean;
  activityId: number;
}

/* ---------- Smart Care ---------- */
export type Area = "freeSpace" | "diskHealth" | "memory" | "battery" | "maintenance" | "startupLoad" | "protection" | "driveLayout";
export type Status = "good" | "warning" | "critical";
export type Verdict = "excellent" | "good" | "fair" | "needsAttention";
export type DiskHealth = "healthy" | "warning" | "unhealthy" | "unknown";

export type Recommendation =
  | { kind: "replaceDisk"; disk: string }
  | { kind: "freeSpace"; drive: string; freePercent: number }
  | { kind: "restart"; uptimeDays: number }
  | { kind: "closeApps"; loadPercent: number }
  | { kind: "cleanJunk"; bytes: number };

export interface Assessment {
  score: number;
  verdict: Verdict;
  areas: { area: Area; status: Status }[];
  recommendations: Recommendation[];
}

export type DriveKind = "fixed" | "removable" | "network" | "optical" | "ram" | "unknown";

export interface DriveInfo {
  letter: string;
  root: string;
  label: string;
  fileSystem: string;
  kind: DriveKind;
  totalBytes: number;
  freeBytes: number;
  isSystem: boolean;
}

export interface MemoryInfo {
  loadPercent: number;
  totalBytes: number;
  availableBytes: number;
  commitPercent: number;
}

export interface PhysicalDisk {
  name: string;
  mediaType: string;
  busType: string;
  sizeBytes: number;
  health: DiskHealth;
  operationalStatus: string;
  letters: string[];
}

export interface SmartCareResult {
  assessment: Assessment;
  systemDrive: DriveInfo | null;
  memory: MemoryInfo | null;
  memoryMinLoad: number | null;
  uptimeSeconds: number;
  disks: PhysicalDisk[] | null;
  hasBattery: boolean;
  cleanup: CleanupScanResult;
  durationMs: number;
  activityId: number;
}

/* ---------- Space Lens ---------- */
export type NodeKind = "dir" | "file" | "others";

export interface ViewNode {
  id: number;
  name: string;
  kind: NodeKind;
  size: number;
  files: number;
  denied: boolean;
  childCount: number;
  children: ViewNode[];
  hiddenCount: number;
  hiddenSize: number;
}

export interface Crumb {
  id: number;
  name: string;
}

export interface SpaceView {
  scanId: number;
  node: ViewNode;
  crumbs: Crumb[];
  path: string | null;
}

export interface ExtStat {
  ext: string;
  bytes: number;
  files: number;
}

export interface SpaceSummary {
  cancelled: boolean;
  rootPath: string;
  view: SpaceView | null;
  extensions: ExtStat[];
  files: number;
  dirs: number;
  bytes: number;
  denied: number;
  durationMs: number;
  drive: DriveInfo | null;
}

export interface SpaceProgress {
  files: number;
  dirs: number;
  bytes: number;
  denied: number;
  current: string;
}

/* ---------- My Activity ---------- */
export type ActivityKind = "smartCareScan" | "cleanupScan" | "cleanup" | "emptyRecycleBin" | "spaceLensScan" | "restore";

export interface ActivityEntry {
  id: number;
  timeMs: number;
  kind: ActivityKind;
  bytes: number;
  items: number;
  failed: number;
  score: number | null;
  target: string | null;
  mode: DeleteMode | null;
  undo: "none" | "available" | "done";
}

export interface RestoreReport {
  restored: number;
  missing: number;
  failed: number;
}

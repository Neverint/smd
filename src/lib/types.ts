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
  | { kind: "cleanJunk"; bytes: number }
  | { kind: "installSecurityUpdates"; count: number }
  | { kind: "installWindowsUpdates"; count: number }
  | { kind: "updateApps"; count: number }
  | { kind: "reviewStartup"; count: number }
  | { kind: "resolveThreats"; count: number }
  | { kind: "turnOnRealTimeProtection" }
  | { kind: "updateSignatures"; days: number }
  | { kind: "checkBattery"; healthPercent: number };

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
  startupEnabled: number | null;
  appUpdates: number | null;
  windowsUpdates: WindowsUpdates | null;
  defender: DefenderStatus | null;
  activeThreats: number | null;
  batteryHealth: number | null;
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
export type ActivityKind =
  | "smartCareScan"
  | "cleanupScan"
  | "cleanup"
  | "emptyRecycleBin"
  | "spaceLensScan"
  | "restore"
  | "startupChange"
  | "uninstall"
  | "leftoversRemoved"
  | "appUpdate"
  | "updatesSetup"
  | "clutterRemoved"
  | "defenderScan"
  | "signatureUpdate"
  | "permissionChange";

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
  enabled?: boolean | null;
  undo: "none" | "available" | "done";
}

export interface RestoreReport {
  restored: number;
  missing: number;
  failed: number;
}

/* ---------- Performance ---------- */
export type StartupSource = "userRun" | "machineRun" | "machineRun32" | "userFolder" | "commonFolder";

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  program: string | null;
  publisher: string | null;
  microsoft: boolean;
  source: StartupSource;
  enabled: boolean;
  needsAdmin: boolean;
}

export interface ProgramUsage {
  name: string;
  processes: number;
  cpuPercent: number;
  memoryBytes: number;
}

export interface ProcessSnapshot {
  byCpu: ProgramUsage[];
  byMemory: ProgramUsage[];
  cpuPercent: number;
  processCount: number;
}

export interface ScheduledTask {
  name: string;
  path: string;
  state: string;
  author: string;
  program: string | null;
  publisher: string | null;
}

export interface Service {
  name: string;
  displayName: string;
  state: string;
  startMode: string;
  program: string | null;
  publisher: string | null;
}

export interface BootInfo {
  lastBootMs: number;
  bootDurationMs: number | null;
  mainPathMs: number | null;
}

export interface Background {
  tasks: ScheduledTask[] | null;
  services: Service[] | null;
  boot: BootInfo | null;
}

export type SystemTool = "taskScheduler" | "services" | "windowsUpdate" | "windowsSecurity" | "privacySettings";

/* ---------- Applications ---------- */
export interface InstalledApp {
  id: string;
  kind: "desktop" | "store";
  name: string;
  publisher: string;
  version: string;
  installDate: string | null;
  sizeBytes: number | null;
  installLocation: string | null;
  microsoft: boolean;
  machineWide: boolean;
}

export interface UninstallResult {
  exitCode: number | null;
  stillInstalled: boolean;
}

export interface UninstallOutcome {
  id: string;
  name: string;
  result: UninstallResult | null;
  error: string | null;
}

export interface IndexProgress {
  index: number;
  total: number;
  name?: string;
  id?: string;
}

export type Confidence = "low" | "medium" | "high";

export interface Leftover {
  path: string;
  kind: "folder" | "registryKey";
  bytes: number;
  confidence: Confidence;
  needsAdmin: boolean;
  selected: boolean;
}

export interface LeftoversView {
  scanId: number;
  appName: string;
  items: Leftover[];
}

export interface RemovalReport {
  removedBytes: number;
  removedItems: number;
  failures: { path: string; reason: FailReason }[];
  undoAvailable: boolean;
}

export interface UpdatesStatus {
  moduleInstalled: boolean;
}

export interface AppUpdate {
  id: string;
  name: string;
  installed: string;
  available: string;
  source: string;
}

export interface UpdateOutcome {
  id: string;
  result: { status: string; rebootRequired: boolean; installerErrorCode: number | null } | null;
  error: string | null;
}

export interface WindowsUpdates {
  total: number;
  security: number;
  titles: string[];
}

/* ---------- My Clutter ---------- */
export interface ClutterFile {
  path: string;
  name: string;
  bytes: number;
  modifiedMs: number | null;
}

export interface DuplicateGroup {
  bytes: number;
  files: ClutterFile[];
}

export interface ClutterResult {
  large: ClutterFile[];
  old: ClutterFile[];
  duplicates: DuplicateGroup[];
  duplicateBytes: number;
  filesScanned: number;
  bytesScanned: number;
  cancelled: boolean;
}

export interface ClutterView {
  scanId: number;
  roots: string[];
  result: ClutterResult;
  durationMs: number;
}

export interface ClutterProgress {
  stage: "walk" | "compare";
  files: number;
  bytes: number;
  hashed: number;
  toHash: number;
}

export interface FolderItem {
  path: string;
  name: string;
  bytes: number;
  files: number;
  modifiedMs: number | null;
  isDir: boolean;
}

export interface DownloadsView {
  scanId: number;
  path: string;
  items: FolderItem[];
}

/* ---------- Protection ---------- */
export interface DefenderStatus {
  serviceEnabled: boolean;
  antivirusEnabled: boolean;
  realTime: boolean;
  tamperProtected: boolean | null;
  signaturesUpdatedMs: number | null;
  signatureAgeDays: number | null;
  quickScanEndMs: number | null;
  fullScanEndMs: number | null;
  runningMode: string;
}

export interface Threat {
  name: string;
  severity: string;
  detectedMs: number | null;
  status: string;
  active: boolean;
  resources: string[];
}

export interface FirewallProfile {
  name: string;
  enabled: boolean;
}

export type Capability = "webcam" | "microphone" | "location";

export interface PermissionEntry {
  capability: Capability;
  id: string;
  app: string;
  packaged: boolean;
  allowed: boolean;
  lastUsedMs: number | null;
  inUse: boolean;
}

export interface CapabilityState {
  capability: Capability;
  allowed: boolean;
  apps: PermissionEntry[];
}

export interface ProtectionView {
  defender: DefenderStatus | null;
  threats: Threat[] | null;
  firewall: FirewallProfile[] | null;
  permissions: CapabilityState[];
}

export type ScanKind = "quick" | "full";

export interface DefenderScanResult {
  cancelled: boolean;
  threatsFound: boolean;
  exitCode: number | null;
  durationMs: number;
  threats: Threat[] | null;
}

/* ---------- Hardware Health ---------- */
export interface CpuInfo {
  name: string;
  cores: number;
  threads: number;
  maxMhz: number;
}

export interface MemoryModule {
  slot: string;
  bank: string;
  capacityBytes: number;
  speedMts: number;
  partNumber: string;
  manufacturer: string;
  kind: string;
}

export interface SystemInfo {
  cpu: CpuInfo | null;
  gpus: { name: string; driverVersion: string }[];
  modules: MemoryModule[];
  slots: number | null;
}

export interface BatteryInfo {
  name: string;
  designCapacity: number;
  fullChargeCapacity: number;
  cycleCount: number | null;
}

export interface HardwareView {
  system: SystemInfo | null;
  disks: PhysicalDisk[] | null;
  memory: MemoryInfo | null;
  batteries: BatteryInfo[];
  hasBattery: boolean;
}

export interface SensorReading {
  name: string;
  kind: string;
  value: number;
}

export interface HardwareReading {
  name: string;
  kind: string;
  sensors: SensorReading[];
}

export interface SensorsView {
  running: boolean;
  exitCode: number | null;
  snapshot: { elevated: boolean; hardware: HardwareReading[]; error: string | null } | null;
}

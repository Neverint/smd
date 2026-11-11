import type { FluentIcon } from "@fluentui/react-icons";
import {
  Apps24Regular,
  Broom24Regular,
  DataPie24Regular,
  DeveloperBoard24Regular,
  DocumentCopy24Regular,
  History24Regular,
  Options24Regular,
  Settings24Regular,
  ShieldCheckmark24Regular,
  Sparkle24Regular,
  Storage24Regular,
  Toolbox24Regular,
  TopSpeed24Regular,
} from "@fluentui/react-icons";

export type ModuleId =
  | "smart-care"
  | "cleanup"
  | "protection"
  | "performance"
  | "applications"
  | "clutter"
  | "space-lens"
  | "hardware"
  | "tweaks"
  | "drive-organizer"
  | "toolbox"
  | "activity"
  | "settings";

export interface ModuleDef {
  id: ModuleId;
  path: string;
  icon: FluentIcon;
  /** Delivery phase that builds this module (see docs/DECISIONS.md). */
  phase: string;
  /** "main" modules are listed at the top of the sidebar, "footer" ones at the bottom. */
  group: "main" | "footer";
  /** Has its own accent colors in module-accents.css. */
  accent: boolean;
}

export const MODULES: ModuleDef[] = [
  { id: "smart-care", path: "/", icon: Sparkle24Regular, phase: "1", group: "main", accent: true },
  { id: "cleanup", path: "/cleanup", icon: Broom24Regular, phase: "1", group: "main", accent: true },
  { id: "protection", path: "/protection", icon: ShieldCheckmark24Regular, phase: "3", group: "main", accent: true },
  { id: "performance", path: "/performance", icon: TopSpeed24Regular, phase: "2", group: "main", accent: true },
  { id: "applications", path: "/applications", icon: Apps24Regular, phase: "2", group: "main", accent: true },
  { id: "clutter", path: "/clutter", icon: DocumentCopy24Regular, phase: "2", group: "main", accent: true },
  { id: "space-lens", path: "/space-lens", icon: DataPie24Regular, phase: "1", group: "main", accent: true },
  { id: "hardware", path: "/hardware", icon: DeveloperBoard24Regular, phase: "3", group: "main", accent: true },
  { id: "tweaks", path: "/tweaks", icon: Options24Regular, phase: "4", group: "main", accent: true },
  { id: "drive-organizer", path: "/drive-organizer", icon: Storage24Regular, phase: "3b", group: "main", accent: true },
  { id: "toolbox", path: "/toolbox", icon: Toolbox24Regular, phase: "4", group: "main", accent: true },
  { id: "activity", path: "/activity", icon: History24Regular, phase: "1", group: "footer", accent: false },
  { id: "settings", path: "/settings", icon: Settings24Regular, phase: "0", group: "footer", accent: false },
];

export function moduleById(id: ModuleId): ModuleDef {
  const def = MODULES.find((m) => m.id === id);
  if (!def) throw new Error(`unknown module ${id}`);
  return def;
}

export function moduleForPath(pathname: string): ModuleDef | undefined {
  return MODULES.find((m) => m.path === pathname);
}

import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { createHashRouter, Outlet, RouterProvider, useLocation } from "react-router";
import { call } from "../lib/backend";
import { useApplyTheme } from "../lib/theme";
import { GalleryPage } from "../dev/gallery/GalleryPage";
import { ActivityPage } from "../modules/activity/ActivityPage";
import { ApplicationsPage } from "../modules/applications/ApplicationsPage";
import { CleanupPage } from "../modules/cleanup/CleanupPage";
import { DriveOrganizerPage } from "../modules/drive-organizer/DriveOrganizerPage";
import { HardwarePage } from "../modules/hardware/HardwarePage";
import { ClutterPage } from "../modules/clutter/ClutterPage";
import { ModulePage } from "../modules/ModulePage";
import { PerformancePage } from "../modules/performance/PerformancePage";
import { ProtectionPage } from "../modules/protection/ProtectionPage";
import { SettingsPage } from "../modules/settings/SettingsPage";
import { SmartCarePage } from "../modules/smart-care/SmartCarePage";
import { SpaceLensPage } from "../modules/space-lens/SpaceLensPage";
import { ToolboxPage } from "../modules/toolbox/ToolboxPage";
import { TrayPage } from "../modules/tray/TrayPage";
import { TweaksPage } from "../modules/tweaks/TweaksPage";
import { MODULES, moduleForPath } from "./modules";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import "./layout.css";

function Shell() {
  const { pathname } = useLocation();
  const current = moduleForPath(pathname);
  return (
    <div className="app">
      <TitleBar />
      <div className="shell">
        <Sidebar />
        <main className="content" data-module={current?.accent ? current.id : undefined}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}

/** Modules with their own page; the others show the "planned for phase N" placeholder. */
const BUILT = new Set(["smart-care", "cleanup", "space-lens", "activity", "settings", "performance", "applications", "clutter", "protection", "hardware", "drive-organizer", "tweaks", "toolbox"]);

const router = createHashRouter([
  {
    element: <Shell />,
    children: [
      { index: true, element: <SmartCarePage /> },
      { path: "cleanup", element: <CleanupPage /> },
      { path: "space-lens", element: <SpaceLensPage /> },
      { path: "activity", element: <ActivityPage /> },
      { path: "performance", element: <PerformancePage /> },
      { path: "applications", element: <ApplicationsPage /> },
      { path: "clutter", element: <ClutterPage /> },
      { path: "protection", element: <ProtectionPage /> },
      { path: "hardware", element: <HardwarePage /> },
      { path: "drive-organizer", element: <DriveOrganizerPage /> },
      { path: "tweaks", element: <TweaksPage /> },
      { path: "toolbox", element: <ToolboxPage /> },
      { path: "settings", element: <SettingsPage /> },
      ...MODULES.filter((m) => !BUILT.has(m.id)).map((m) => ({
        path: m.path.slice(1),
        element: <ModulePage id={m.id} />,
      })),
      { path: "dev/gallery", element: <GalleryPage /> },
    ],
  },
  // The tray icon's popover window (no title bar or sidebar).
  { path: "tray", element: <TrayPage /> },
]);

export function App() {
  useApplyTheme();
  const { i18n } = useTranslation();
  const lang = i18n.resolvedLanguage?.startsWith("fr") ? "fr" : "en";

  // The backend writes notifications and the tray menu in the UI language.
  useEffect(() => {
    call("settings_get")
      .then((s) => (s.language !== lang ? call("settings_set", { settings: { ...s, language: lang } }) : s))
      .catch(() => undefined);
  }, [lang]);

  // Make the page transparent only once the backend confirms Mica or acrylic is on.
  useEffect(() => {
    call("app_info")
      .then((info) => document.documentElement.classList.toggle("has-window-effect", info.windowEffect !== "none"))
      .catch(() => undefined);
  }, []);

  return (
    <MotionConfig reducedMotion="user">
      <RouterProvider router={router} />
    </MotionConfig>
  );
}

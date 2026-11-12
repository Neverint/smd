import { MotionConfig } from "motion/react";
import { lazy, Suspense, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { createHashRouter, Outlet, RouterProvider, useLocation } from "react-router";
import { call } from "../lib/backend";
import { useApplyTheme } from "../lib/theme";
import { ModulePage } from "../modules/ModulePage";
import { SmartCarePage } from "../modules/smart-care/SmartCarePage";
import { MODULES, moduleForPath } from "./modules";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import "./layout.css";

// Smart Care (the home page) loads with the app; every other page loads when it is first opened.
const ActivityPage = lazy(() => import("../modules/activity/ActivityPage").then((m) => ({ default: m.ActivityPage })));
const ApplicationsPage = lazy(() => import("../modules/applications/ApplicationsPage").then((m) => ({ default: m.ApplicationsPage })));
const CleanupPage = lazy(() => import("../modules/cleanup/CleanupPage").then((m) => ({ default: m.CleanupPage })));
const ClutterPage = lazy(() => import("../modules/clutter/ClutterPage").then((m) => ({ default: m.ClutterPage })));
const DriveOrganizerPage = lazy(() => import("../modules/drive-organizer/DriveOrganizerPage").then((m) => ({ default: m.DriveOrganizerPage })));
const GalleryPage = lazy(() => import("../dev/gallery/GalleryPage").then((m) => ({ default: m.GalleryPage })));
const HardwarePage = lazy(() => import("../modules/hardware/HardwarePage").then((m) => ({ default: m.HardwarePage })));
const PerformancePage = lazy(() => import("../modules/performance/PerformancePage").then((m) => ({ default: m.PerformancePage })));
const ProtectionPage = lazy(() => import("../modules/protection/ProtectionPage").then((m) => ({ default: m.ProtectionPage })));
const SettingsPage = lazy(() => import("../modules/settings/SettingsPage").then((m) => ({ default: m.SettingsPage })));
const SpaceLensPage = lazy(() => import("../modules/space-lens/SpaceLensPage").then((m) => ({ default: m.SpaceLensPage })));
const ToolboxPage = lazy(() => import("../modules/toolbox/ToolboxPage").then((m) => ({ default: m.ToolboxPage })));
const TrayPage = lazy(() => import("../modules/tray/TrayPage").then((m) => ({ default: m.TrayPage })));
const TweaksPage = lazy(() => import("../modules/tweaks/TweaksPage").then((m) => ({ default: m.TweaksPage })));

function Shell() {
  const { pathname } = useLocation();
  const current = moduleForPath(pathname);
  return (
    <div className="app">
      <TitleBar />
      <div className="shell">
        <Sidebar />
        <main className="content" data-module={current?.accent ? current.id : undefined}>
          <Suspense fallback={null}>
            <Outlet />
          </Suspense>
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
  {
    path: "tray",
    element: (
      <Suspense fallback={null}>
        <TrayPage />
      </Suspense>
    ),
  },
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

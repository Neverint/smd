import { MotionConfig } from "motion/react";
import { useEffect } from "react";
import { createHashRouter, Outlet, RouterProvider, useLocation } from "react-router";
import { call } from "../lib/backend";
import { useApplyTheme } from "../lib/theme";
import { GalleryPage } from "../dev/gallery/GalleryPage";
import { ActivityPage } from "../modules/activity/ActivityPage";
import { CleanupPage } from "../modules/cleanup/CleanupPage";
import { ModulePage } from "../modules/ModulePage";
import { SettingsPage } from "../modules/settings/SettingsPage";
import { SmartCarePage } from "../modules/smart-care/SmartCarePage";
import { SpaceLensPage } from "../modules/space-lens/SpaceLensPage";
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
const BUILT = new Set(["smart-care", "cleanup", "space-lens", "activity", "settings"]);

const router = createHashRouter([
  {
    element: <Shell />,
    children: [
      { index: true, element: <SmartCarePage /> },
      { path: "cleanup", element: <CleanupPage /> },
      { path: "space-lens", element: <SpaceLensPage /> },
      { path: "activity", element: <ActivityPage /> },
      { path: "settings", element: <SettingsPage /> },
      ...MODULES.filter((m) => !BUILT.has(m.id)).map((m) => ({
        path: m.path.slice(1),
        element: <ModulePage id={m.id} />,
      })),
      { path: "dev/gallery", element: <GalleryPage /> },
    ],
  },
]);

export function App() {
  useApplyTheme();

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

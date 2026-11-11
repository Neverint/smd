import { defineConfig } from "@playwright/test";

/**
 * Smoke tests against the Vite dev server. Outside Tauri the frontend uses the
 * mocked backend (src/lib/mocks.ts), so nothing touches the real system.
 * Uses the installed Microsoft Edge (present on every Windows 10/11 PC and CI runner).
 */
export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:1420",
    channel: "msedge",
    locale: "en-US",
    colorScheme: "light",
  },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});

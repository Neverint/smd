import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// Accessibility check (WCAG 2.1 A and AA rules) of every page, in light and dark, with the
// mocked backend. Pages that start with a scan are scanned first so results are checked too.

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    (window as unknown as { __SMD_FAST_MOCKS__: boolean }).__SMD_FAST_MOCKS__ = true;
  });
});

interface PageCase {
  path: string;
  /** Brings the page to its results state. */
  prepare?: (page: Page) => Promise<void>;
}

const scan = async (page: Page) => {
  await page.getByRole("button", { name: "Scan", exact: true }).click();
};

const PAGES: PageCase[] = [
  { path: "/", prepare: async (p) => (await scan(p), await p.getByRole("progressbar", { name: /Health score/ }).waitFor()) },
  { path: "/#/cleanup", prepare: async (p) => (await scan(p), await p.getByText("can be removed").waitFor()) },
  { path: "/#/space-lens", prepare: async (p) => (await p.getByRole("button", { name: /^C: Windows/ }).click(), await p.getByRole("heading", { name: "Largest items" }).waitFor()) },
  { path: "/#/protection" },
  { path: "/#/performance" },
  { path: "/#/applications" },
  { path: "/#/clutter" },
  { path: "/#/hardware" },
  { path: "/#/tweaks", prepare: async (p) => void (await p.locator(".cat-list li").first().waitFor()) },
  { path: "/#/drive-organizer" },
  { path: "/#/toolbox", prepare: async (p) => void (await p.locator(".toolbox-card").first().waitFor()) },
  { path: "/#/activity" },
  { path: "/#/settings" },
  { path: "/#/tray" },
];

for (const theme of ["light", "dark"] as const) {
  for (const c of PAGES) {
    test(`${theme}: ${c.path} has no WCAG A/AA violations`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await page.goto(c.path);
      await page.waitForLoadState("networkidle");
      if (c.prepare) await c.prepare(page).catch(() => undefined);
      await page.waitForTimeout(300);
      const results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]).analyze();
      const summary = results.violations.map((v) => `${v.id} (${v.impact}): ${v.nodes.length} × ${v.nodes[0]?.target.join(" ")} — ${v.nodes[0]?.failureSummary?.split("\n")[1] ?? ""}`);
      expect(summary, summary.join("\n")).toEqual([]);
    });
  }
}

test("dialogs and side sheets have no WCAG A/AA violations", async ({ page }) => {
  await page.goto("/#/cleanup");
  await page.getByRole("button", { name: "Scan", exact: true }).click();
  await page.getByRole("button", { name: "Review: Your temporary files" }).click();
  await page.getByRole("dialog").first().waitFor();
  let results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]).analyze();
  expect(results.violations.map((v) => v.id)).toEqual([]);

  await page.goto("/#/tweaks");
  await page.getByRole("checkbox", { name: "Turn off activity history" }).check();
  await page.getByRole("button", { name: "Apply 1 change" }).click();
  await page.getByRole("dialog", { name: "Apply 1 change?" }).waitFor();
  results = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]).analyze();
  expect(results.violations.map((v) => v.id)).toEqual([]);
});
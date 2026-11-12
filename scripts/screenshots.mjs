// README screenshots from the mocked frontend (no real PC data). Start `npm run dev` first, then:
//   node scripts/screenshots.mjs
// Writes docs/screenshots/*.png at the app's default window size.
import { chromium } from "@playwright/test";
import fs from "node:fs";

const BASE = process.env.SMD_URL ?? "http://localhost:1420";
const OUT = "docs/screenshots";
fs.mkdirSync(OUT, { recursive: true });

const browser = await chromium.launch({ channel: "msedge" });

async function shoot(name, theme, steps) {
  const page = await browser.newPage({ viewport: { width: 1120, height: 740 }, deviceScaleFactor: 1, colorScheme: theme, reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.__SMD_FAST_MOCKS__ = true;
    localStorage.setItem("smd.settings", JSON.stringify({ state: { theme: "system", language: "en" }, version: 1 }));
  });
  await steps(page);
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${OUT}/${name}.png` });
  await page.close();
  console.log("saved", name);
}

await shoot("smart-care", "light", async (p) => {
  await p.goto(BASE + "/");
  await p.getByRole("button", { name: "Scan", exact: true }).click();
  await p.getByRole("progressbar", { name: /Health score/ }).waitFor();
});
await shoot("cleanup", "light", async (p) => {
  await p.goto(BASE + "/#/cleanup");
  await p.getByRole("button", { name: "Scan", exact: true }).click();
  await p.getByText("can be removed").waitFor();
});
await shoot("space-lens", "dark", async (p) => {
  await p.goto(BASE + "/#/space-lens");
  await p.getByRole("button", { name: /^C: Windows/ }).click();
  await p.getByRole("heading", { name: "Largest items" }).waitFor();
});
await shoot("drive-organizer", "light", async (p) => {
  await p.goto(BASE + "/#/drive-organizer");
  await p.getByText("Elden Ring").first().waitFor();
});
await shoot("tweaks", "dark", async (p) => {
  await p.goto(BASE + "/#/tweaks");
  await p.locator(".cat-list li").first().waitFor();
  // Scroll the list (not the window) so the action bar stays in view.
  await p.locator("main.content").evaluate((el) => (el.scrollTop = 380));
  await p.getByRole("checkbox", { name: "Turn off activity history" }).check();
  await p.getByRole("checkbox", { name: "No tips and tricks pop-ups" }).check();
});
await shoot("toolbox", "light", async (p) => {
  await p.goto(BASE + "/#/toolbox");
  await p.locator(".toolbox-card").first().waitFor();
});

// The tray popover at its own window size.
{
  const page = await browser.newPage({ viewport: { width: 340, height: 470 }, colorScheme: "dark", reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.__SMD_FAST_MOCKS__ = true;
    localStorage.setItem("smd.settings", JSON.stringify({ state: { theme: "system", language: "en" }, version: 1 }));
  });
  await page.goto(BASE + "/#/tray");
  await page.waitForTimeout(30000);
  await page.screenshot({ path: `${OUT}/tray.png` });
  console.log("saved tray");
}
await browser.close();

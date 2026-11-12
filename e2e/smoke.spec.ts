import { expect, test } from "@playwright/test";

// The mocked backend skips its artificial delays.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    (window as unknown as { __SMD_FAST_MOCKS__: boolean }).__SMD_FAST_MOCKS__ = true;
  });
});

test("home shows Smart Care and the full sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1, name: "Smart Care" })).toBeVisible();
  const nav = page.getByRole("navigation", { name: "Modules" });
  await expect(nav.getByRole("link")).toHaveCount(13);
  await expect(page.getByRole("button", { name: "Scan", exact: true })).toBeEnabled();
});

test("Smart Care scans, scores and cleans the default junk", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Scan", exact: true }).click();
  await expect(page.getByRole("progressbar", { name: /Health score: 82/ })).toBeVisible();
  await expect(page.getByText("Restart your PC: it has been on for 34 days.")).toBeVisible();
  await page.getByRole("button", { name: /^Clean / }).click();
  await expect(page.getByRole("heading", { name: "All done" })).toBeVisible();
  await expect(page.getByText(/moved to the Recycle Bin/)).toBeVisible();
});

test("Cleanup review, skip and confirm before emptying the Recycle Bin", async ({ page }) => {
  await page.goto("/#/cleanup");
  await page.getByRole("button", { name: "Scan", exact: true }).click();
  await expect(page.getByText("can be removed")).toBeVisible();

  // Admin-only categories cannot be ticked.
  await expect(page.getByRole("checkbox", { name: "Windows Update downloads" })).toBeDisabled();

  // Untick one item in the review sheet: the total goes down.
  const button = page.getByRole("button", { name: /^Clean / });
  const before = await button.textContent();
  await page.getByRole("button", { name: "Review: Your temporary files" }).click();
  const sheet = page.getByRole("dialog", { name: "Review: Your temporary files" });
  await sheet.getByRole("checkbox", { name: /item-1/ }).uncheck();
  await sheet.getByRole("button", { name: "Done" }).click();
  await expect(button).not.toHaveText(before ?? "");

  // Emptying the Recycle Bin asks first; Cancel changes nothing.
  await page.getByRole("checkbox", { name: "Recycle Bin" }).check();
  await button.click();
  const confirm = page.getByRole("dialog", { name: "Empty the Recycle Bin?" });
  await expect(confirm).toBeVisible();
  await expect(confirm.getByRole("button", { name: "Cancel" })).toBeFocused();
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByText("can be removed")).toBeVisible();

  await button.click();
  await page.getByRole("dialog", { name: "Empty the Recycle Bin?" }).getByRole("button", { name: "Delete" }).click();
  await expect(page.getByRole("heading", { name: "Cleanup finished" })).toBeVisible();
  await expect(page.getByText(/The Recycle Bin was emptied/)).toBeVisible();

  // My Activity lists the cleanup with an Undo.
  await page.getByRole("link", { name: "Open My Activity" }).click();
  await page.getByRole("button", { name: "Undo: Cleanup" }).click();
  await expect(page.getByRole("status")).toContainText("put back");
  await expect(page.getByText("Undone")).toBeVisible();
});

test("Space Lens scans a drive and drills into folders", async ({ page }) => {
  await page.goto("/#/space-lens");
  await page.getByRole("button", { name: /^C: Windows/ }).click();
  await expect(page.getByRole("heading", { name: "Largest items" })).toBeVisible();
  await page.getByRole("group", { name: /Map of C:/ }).getByRole("button", { name: "Open Users" }).click();
  const crumbs = page.getByRole("navigation", { name: "Location" });
  await expect(crumbs.getByRole("button", { name: "Users" })).toHaveAttribute("aria-current", "location");
  await page.getByRole("button", { name: "Up one level" }).click();
  await expect(crumbs.getByRole("button", { name: "C:\\" })).toHaveAttribute("aria-current", "location");
});

test("Performance: turn a startup item off, machine-wide items stay locked, undo in My Activity", async ({ page }) => {
  await page.goto("/#/performance");
  const discord = page.getByRole("switch", { name: "Start Discord with Windows" });
  await expect(discord).toHaveAttribute("aria-checked", "true");
  await discord.click();
  await expect(discord).toHaveAttribute("aria-checked", "false");
  await expect(page.getByRole("switch", { name: "Start RtkAudUService with Windows" })).toBeDisabled();

  await page.getByRole("tab", { name: "Running now" }).click();
  await expect(page.getByRole("region", { name: "Using memory" })).toContainText("msedge.exe");
  await page.getByRole("tab", { name: "Background" }).click();
  await expect(page.getByText("Steam Client Service")).toBeVisible();

  await page.goto("/#/activity");
  await expect(page.getByText("Discord no longer starts with Windows")).toBeVisible();
});

test("Applications: uninstall asks first, then offers a leftovers review", async ({ page }) => {
  await page.goto("/#/applications");
  await page.getByRole("checkbox", { name: "Discord" }).check();
  await page.getByRole("button", { name: "Uninstall 1 app" }).click();
  const confirm = page.getByRole("dialog", { name: "Uninstall these apps?" });
  await expect(confirm).toContainText("Discord");
  await confirm.getByRole("button", { name: "Uninstall" }).click();
  await expect(page.getByText("Uninstall finished")).toBeVisible();
  await page.getByRole("button", { name: "Look for leftovers" }).click();
  const sheet = page.getByRole("dialog", { name: "Leftovers of Discord" });
  // Only the app's own folder is ticked by default.
  await expect(sheet.getByRole("checkbox", { name: /Local\\Discord/ })).toBeChecked();
  await expect(sheet.getByRole("checkbox", { name: /Roaming\\Discord/ })).not.toBeChecked();
  await sheet.getByRole("button", { name: "Remove 1 item" }).click();
  await expect(sheet.getByRole("status")).toContainText("Undo in My Activity");

  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Updates" }).click();
  await expect(page.getByText("2 updates are waiting (1 security).")).toBeVisible();
  await page.getByRole("button", { name: "Download and set up" }).click();
  await expect(page.getByRole("button", { name: "Update 2 apps" })).toBeVisible();
});

test("My Clutter keeps at least one copy of each duplicate", async ({ page }) => {
  await page.goto("/#/clutter");
  await page.getByRole("button", { name: "Scan your folders" }).click();
  await page.getByRole("tab", { name: /Duplicates/ }).click();
  await page.getByRole("button", { name: "Select all but one copy" }).click();
  const kept = page.getByRole("checkbox", { name: "G:\\Pictures\\trip\\IMG_0001.mov" });
  await expect(kept).not.toBeChecked();
  await expect(kept).toBeDisabled();
  await page.getByRole("button", { name: /^Move 3 items to the Recycle Bin/ }).click();
  await expect(page.getByRole("heading", { name: "Moved to the Recycle Bin" })).toBeVisible();
});

test("Protection: status, a quick scan, and a Store app permission with undo", async ({ page }) => {
  await page.goto("/#/protection");
  await expect(page.getByText("Microsoft Defender and the firewall are on.")).toBeVisible();
  await page.getByRole("button", { name: "Scan now" }).click();
  await expect(page.getByRole("status")).toContainText("No threats found");
  await expect(page.getByText("PUA:Win32/Presenoker")).toBeVisible();

  await page.getByRole("tab", { name: "Firewall" }).click();
  await expect(page.getByText("Public network")).toBeVisible();

  await page.getByRole("tab", { name: "App permissions" }).click();
  const camera = page.getByRole("switch", { name: "Allow Microsoft.WindowsCamera to use the Camera" });
  await camera.click();
  await expect(camera).toHaveAttribute("aria-checked", "false");
  await page.getByRole("radio", { name: "Microphone" }).click();
  await expect(page.getByText("In use now")).toBeVisible();
  // Desktop apps have no per-app switch in Windows: shown read-only.
  await expect(page.getByRole("switch", { name: /Discord/ })).toHaveCount(0);

  await page.goto("/#/activity");
  await expect(page.getByText("Microsoft.WindowsCamera can no longer use the Camera")).toBeVisible();
});

test("Hardware Health: live sensors, drives and memory", async ({ page }) => {
  await page.goto("/#/hardware");
  await expect(page.getByRole("region", { name: "AMD Radeon RX 6700 XT" })).toContainText("°C");
  await expect(page.getByText("Processor temperature and motherboard fans need admin rights")).toBeVisible();
  await expect(page.getByText("X: Samsung SSD 990 PRO with Heatsink 1TB")).toBeVisible();
  await expect(page.getByText("2 of 4 slots used")).toBeVisible();
  await expect(page.getByText("Battery", { exact: true })).toHaveCount(0);
});

test("Settings: permanent delete shows a warning", async ({ page }) => {
  await page.goto("/#/settings");
  await page.getByRole("radio", { name: "Delete for good" }).click();
  await expect(page.getByText("Permanent delete skips the Recycle Bin")).toBeVisible();
  await page.getByRole("radio", { name: "Recycle Bin" }).click();
  await expect(page.getByText("Permanent delete skips the Recycle Bin")).toBeHidden();
});

test("navigates to a module page", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Space Lens" }).click();
  await expect(page).toHaveURL(/#\/space-lens$/);
  await expect(page.getByRole("heading", { level: 1, name: "Space Lens" })).toBeVisible();
  await expect(page.locator("main")).toHaveAttribute("data-module", "space-lens");
});

test("switches language to French and theme to dark", async ({ page }) => {
  await page.goto("/#/settings");
  await page.getByRole("radio", { name: "Français" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Paramètres" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "fr");

  await page.getByRole("radio", { name: "Sombre" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  // Preferences survive a reload.
  await page.reload();
  await expect(page.getByRole("link", { name: "Nettoyage" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

test("sidebar collapses and keeps accessible names", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Collapse navigation" }).click();
  await expect(page.getByRole("button", { name: "Expand navigation" })).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByRole("link", { name: "Cleanup" })).toBeVisible();
});

test("gallery renders both themes and the side sheet traps Escape", async ({ page }) => {
  await page.goto("/#/dev/gallery");
  await expect(page.locator('[data-theme="light"].gallery__panel')).toBeVisible();
  await expect(page.locator('[data-theme="dark"].gallery__panel')).toBeVisible();

  await page.getByRole("button", { name: "Open side sheet" }).first().click();
  const dialog = page.getByRole("dialog", { name: "Review: Cleanup" });
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

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
  await page.getByRole("button", { name: "Undo this cleanup" }).click();
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

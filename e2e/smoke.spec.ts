import { expect, test } from "@playwright/test";

test("home shows Smart Care and the full sidebar", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1, name: "Smart Care" })).toBeVisible();
  const nav = page.getByRole("navigation", { name: "Modules" });
  await expect(nav.getByRole("link")).toHaveCount(13);
  await expect(page.getByRole("button", { name: "Scan" })).toBeDisabled();
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

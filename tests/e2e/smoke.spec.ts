import { expect, test } from "@playwright/test";

test("Startseite zeigt Titel und Claim", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Verschätz dich" })).toBeVisible();
});

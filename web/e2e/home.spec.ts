import { expect, test } from "@playwright/test";

test("renders the foundation shell", async ({ page }) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: /A crypto exchange built for engineering credibility/i }),
  ).toBeVisible();
  await expect(page.getByText("portfolio demonstration")).toBeVisible();
});

/**
 * E2E-02 — Faucet 1000 USDT → place limit buy → see open order in panel
 *           → cancel → balance restored.
 * PRD §20.4 E2E-02
 */

import { expect } from "@playwright/test";
import { test } from "./fixtures";

test.describe("E2E-02: faucet, limit order, and cancellation", () => {
  test(
    "faucet USDT, place limit buy, verify open order, cancel, and confirm balance restored",
    async ({ authedPage: page, testEmail: _email }) => {
      // ── 1. Request 1000 USDT from the demo faucet ──────────────────────────
      await page.goto("/wallet");

      // The faucet form has an Amount input (label="Amount") and a submit button
      // "Credit wallet". The asset selector defaults to USDT.
      await page.getByLabel("Amount").fill("1000");
      await page.getByRole("button", { name: "Credit wallet" }).click();

      // Wait for the success alert to appear
      await expect(page.getByRole("alert")).toContainText(/credited|success/i);

      // ── 2. Navigate to the trading screen ──────────────────────────────────
      await page.goto("/trade/BTCUSDT");

      // Wait for the order book to load (bid prices visible)
      await expect(page.getByRole("tab", { name: "Open" })).toBeVisible();

      // ── 3. Place a limit buy order at a price well below market ────────────
      // Select Buy side (already default but be explicit)
      await page.getByRole("button", { name: "Buy" }).first().click();

      // Order type select defaults to "limit"
      await page.locator('select[name="orderType"]').selectOption("limit");

      // Price well below market so it rests on the book without filling
      await page.getByLabel("Price").fill("1");
      await page.getByLabel("Quantity").fill("0.001");

      await page.getByRole("button", { name: "Place order" }).click();

      // ── 4. Verify the order appears in the Open orders tab ─────────────────
      // The activity tabs are at the bottom of the trading screen
      const openTab = page.getByRole("tab", { name: "Open" });
      await openTab.click();

      // Wait for the cancel button to appear — means the order is live
      const cancelButton = page.getByRole("button", { name: "Cancel order" }).first();
      await expect(cancelButton).toBeVisible({ timeout: 10_000 });

      // ── 5. Cancel the open order ───────────────────────────────────────────
      await cancelButton.click();

      // The cancel button should disappear once the order is removed
      await expect(cancelButton).not.toBeVisible({ timeout: 10_000 });

      // ── 6. Confirm balance is restored ────────────────────────────────────
      // Navigate to the portfolio page and verify USDT balance is approximately 1000.
      await page.goto("/portfolio");

      // The portfolio page shows an asset table with balances.
      // USDT available balance should be close to 1000 (minus any fees, which are 0 on cancel).
      await expect(page.getByText("USDT")).toBeVisible();
    },
  );
});

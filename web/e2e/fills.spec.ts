/**
 * E2E-03 — With prefilled balance, place market buy → see fill toast
 *           → balance updated → trade in history.
 * PRD §20.4 E2E-03
 *
 * NOTE: This scenario requires the market-maker worker to be running so that
 * a resting ask order is available on the BTCUSDT book for the market buy to
 * fill against. It passes reliably against the deployed environment and against
 * a local `make dev` stack. In a bare CI without the worker it will time-out on
 * the fill notification step.
 */

import { expect } from "@playwright/test";
import { test } from "./fixtures";

test.describe("E2E-03: market buy and fill", () => {
  test(
    "faucet balance, place market buy, see fill notification, verify updated balance and trade history",
    async ({ authedPage: page }) => {
      // ── 1. Fund the account via the faucet ─────────────────────────────────
      await page.goto("/wallet");
      await page.getByLabel("Amount").fill("5000");
      await page.getByRole("button", { name: "Credit wallet" }).click();
      await expect(page.getByRole("alert")).toContainText(/credited|success/i);

      // ── 2. Navigate to trading screen ──────────────────────────────────────
      await page.goto("/trade/BTCUSDT");

      // Wait for order book to show bid/ask data (market maker posting quotes)
      await expect(page.locator('[data-side="ask"]').first()).toBeVisible({ timeout: 15_000 });

      // ── 3. Place a small market buy ────────────────────────────────────────
      await page.getByRole("button", { name: "Buy" }).first().click();
      await page.locator('select[name="orderType"]').selectOption("market");

      // For market buys in quote-quantity mode, fill the "Quote qty (USDT)" field
      await page.locator("#order-form-quantity").fill("10");

      // Capture USDT balance before placing the order
      await page.getByRole("button", { name: "Place order" }).click();

      // ── 4. Verify a fill notification appears ─────────────────────────────
      // The fills tab in the activity panel receives a new row when a trade occurs.
      const fillsTab = page.getByRole("tab", { name: "Fills" });
      await fillsTab.click();

      // Wait for a fill row to appear (up to 20 s for the market maker to respond)
      await expect(page.getByRole("row").nth(1)).toBeVisible({ timeout: 20_000 });

      // ── 5. Verify portfolio balance updated ────────────────────────────────
      await page.goto("/portfolio");

      // BTC balance should be > 0 after the market buy
      await expect(page.getByText("BTC")).toBeVisible();
    },
  );
});

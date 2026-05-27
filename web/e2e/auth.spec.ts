/**
 * E2E-01 — Sign up → enable 2FA → log in with 2FA → land on trading screen.
 * PRD §20.4 E2E-01
 */

import { expect, test } from "@playwright/test";
import { generateTotp, signUpAndLogin, TEST_PASSWORD } from "./fixtures";

test.describe("E2E-01: auth with 2FA", () => {
  test("sign up, enable 2FA, log out, then log in using the TOTP code", async ({ page }) => {
    const email = `e2e-auth+${Date.now()}@aether.test`;

    // ── 1. Sign up (auto-logs in and lands on /trade/**) ─────────────────────
    await signUpAndLogin(page, email);
    await expect(page).toHaveURL(/\/trade\//);

    // ── 2. Navigate to account settings ──────────────────────────────────────
    await page.goto("/account");
    await expect(page.getByText("Authenticator app")).toBeVisible();

    // ── 3. Start 2FA setup — intercept the API response to capture the secret ─
    const setupResponsePromise = page.waitForResponse(
      (r) => r.url().includes("/auth/2fa/setup") && r.request().method() === "POST",
    );
    await page.getByRole("button", { name: "Start setup" }).click();

    const setupResponse = await setupResponsePromise;
    const { secret } = (await setupResponse.json()) as { secret: string };
    expect(secret.length).toBeGreaterThan(10);

    // ── 4. The manual key code is visible on the page ─────────────────────────
    await expect(page.locator("code").filter({ hasText: secret })).toBeVisible();

    // ── 5. Generate TOTP code from secret and verify ──────────────────────────
    const totpCode = generateTotp(secret);
    // The code input is labelled "Authenticator code"
    await page.getByLabel("Authenticator code").fill(totpCode);
    await page.getByRole("button", { name: "Verify and enable" }).click();

    // ── 6. Assert 2FA shows "Enabled" ─────────────────────────────────────────
    await expect(page.getByText("Enabled")).toBeVisible();

    // ── 7. Log out via the account dropdown in the TopBar ─────────────────────
    // The TopBar has an account dropdown; find the sign-out trigger.
    await page.goto("/trade/BTCUSDT");
    // Open account menu (button contains the user email or a person icon)
    const accountMenu = page.getByRole("button", { name: /account|sign out|logout/i }).first();
    if (await accountMenu.isVisible()) {
      await accountMenu.click();
      const logoutItem = page.getByRole("menuitem", { name: /sign out|logout/i });
      if (await logoutItem.isVisible()) {
        await logoutItem.click();
      }
    }
    // Fallback: navigate to the login page directly
    await page.goto("/login");
    await expect(page).toHaveURL(/\/login/);

    // ── 8. Log in — triggers MFA redirect ─────────────────────────────────────
    await page.getByLabel("Email").fill(email);
    await page.getByLabel("Password").fill(TEST_PASSWORD);
    await page.getByRole("button", { name: "Sign in" }).click();

    // After submitting credentials, redirected to /2fa?token=...
    await page.waitForURL("**/2fa**");
    await expect(page).toHaveURL(/\/2fa/);

    // ── 9. Enter the TOTP code on the 2FA page ────────────────────────────────
    const freshCode = generateTotp(secret);
    await page.getByLabel("Authenticator code").fill(freshCode);
    await page.getByRole("button", { name: "Verify" }).click();

    // ── 10. Assert: landed on the trading screen ──────────────────────────────
    await page.waitForURL("**/trade/**");
    await expect(page).toHaveURL(/\/trade\//);
  });
});

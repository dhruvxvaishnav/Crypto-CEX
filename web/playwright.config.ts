import { defineConfig, devices } from "@playwright/test";

/**
 * Base URL for the frontend.
 * Override with PLAYWRIGHT_BASE_URL=https://aether-web.vercel.app to run against
 * the deployed environment (PRD §21 Day 13 — Smoke E2E against deployed env).
 */
const BASE_URL = process.env.PLAYWRIGHT_BASE_URL ?? "http://127.0.0.1:3000";
const isDeployed = Boolean(process.env.PLAYWRIGHT_BASE_URL);

export default defineConfig({
  globalSetup: "./e2e/global-setup.ts",
  projects: [
    {
      name: "chromium",
      use: devices["Desktop Chrome"],
    },
  ],
  testDir: "./e2e",
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  // Fail fast on first few spec errors when running in CI.
  ...(process.env.CI ? { maxFailures: 3 } : {}),
  // Only spin up the dev server when running locally against the default stack.
  ...(isDeployed
    ? {}
    : {
        webServer: {
          command: "pnpm --filter @aether/web dev",
          reuseExistingServer: !process.env.CI,
          timeout: 120_000,
          url: "http://127.0.0.1:3000",
        },
      }),
});

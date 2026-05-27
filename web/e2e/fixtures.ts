/**
 * Shared Playwright fixtures and test utilities for Aether E2E specs.
 *
 * - `authedPage`: page fixture that has already completed the signup+login
 *   flow and lands on `/trade/BTCUSDT` for the fresh test user.
 *
 * - `generateTotp`: inline TOTP (RFC 6238 / HOTP-with-time) generator so
 *   E2E-01 can complete the 2FA verify step without an external library.
 *
 * - `buildHmacSignature`: inline HMAC-SHA256 signer matching the payload
 *   format in `services/api/src/extractors/hmac.rs` for E2E-04.
 */

import { createHmac } from "node:crypto";
import { test as base } from "@playwright/test";
import type { Page } from "@playwright/test";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/** Direct Rust API base URL — used for requests that bypass the Next.js proxy. */
export const API_BASE = process.env.API_BASE_URL ?? "http://localhost:8080/api/v1";

/** Strong password that satisfies the signup validator (≥12 chars, upper, digit). */
export const TEST_PASSWORD = "TestPass123!Aether";

// ─────────────────────────────────────────────────────────────────────────────
// TOTP (RFC 6238) — no npm deps
// ─────────────────────────────────────────────────────────────────────────────

function base32Decode(input: string): Buffer {
  const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = 0;
  let value = 0;
  const bytes: number[] = [];
  for (const ch of input.toUpperCase().replace(/[^A-Z2-7]/g, "")) {
    value = (value << 5) | ALPHABET.indexOf(ch);
    bits += 5;
    if (bits >= 8) {
      bytes.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Buffer.from(bytes);
}

/**
 * Generates a 6-digit TOTP code for the given base32 secret.
 * Compatible with the TOTP validator in `services/api/src/handlers/totp.rs`.
 */
export function generateTotp(base32Secret: string): string {
  const counter = Math.floor(Date.now() / 30_000);
  const msg = Buffer.allocUnsafe(8);
  msg.writeBigUInt64BE(BigInt(counter));
  const hash = createHmac("sha1", base32Decode(base32Secret)).update(msg).digest();
  const offset = hash[hash.length - 1]! & 0x0f;
  const code =
    ((hash[offset]! & 0x7f) << 24) |
    (hash[offset + 1]! << 16) |
    (hash[offset + 2]! << 8) |
    hash[offset + 3]!;
  return (code % 1_000_000).toString().padStart(6, "0");
}

// ─────────────────────────────────────────────────────────────────────────────
// HMAC-SHA256 request signer (matches extractors/hmac.rs payload format)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Builds the HMAC-SHA256 hex signature for a signed API request.
 *
 * Payload format (from `services/api/src/extractors/hmac.rs`):
 *   `{timestampMs}\n{METHOD}\n{/path?query}\n{body}`
 */
export function buildHmacSignature(
  secret: string,
  timestampMs: string,
  method: string,
  pathAndQuery: string,
  body: string,
): string {
  const payload = `${timestampMs}\n${method.toUpperCase()}\n${pathAndQuery}\n${body}`;
  return createHmac("sha256", secret).update(payload).digest("hex");
}

// ─────────────────────────────────────────────────────────────────────────────
// Auth helper — signs up and logs in a fresh test user via the UI
// ─────────────────────────────────────────────────────────────────────────────

/** Signs up a fresh user and navigates to the trading screen. */
export async function signUpAndLogin(page: Page, email: string): Promise<void> {
  await page.goto("/signup");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(TEST_PASSWORD);
  await page.getByLabel("Confirm password").fill(TEST_PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  // signup() calls finishAuth() which redirects to /trade/BTCUSDT
  await page.waitForURL("**/trade/**");
}

/**
 * Reads the access token stored in sessionStorage by the auth store.
 * Key: `aether-auth` → `{ state: { accessToken } }`.
 */
export async function getAccessToken(page: Page): Promise<string> {
  const token = await page.evaluate(() => {
    try {
      const raw = sessionStorage.getItem("aether-auth");
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { state?: { accessToken?: string } };
      return parsed.state?.accessToken ?? null;
    } catch {
      return null;
    }
  });
  if (!token) throw new Error("Access token not found in sessionStorage");
  return token;
}

// ─────────────────────────────────────────────────────────────────────────────
// Extended test fixture
// ─────────────────────────────────────────────────────────────────────────────

interface AetherFixtures {
  /** Page authenticated as a fresh test user on `/trade/BTCUSDT`. */
  authedPage: Page;
  /** The email of the currently-authenticated test user. */
  testEmail: string;
}

export const test = base.extend<AetherFixtures>({
  testEmail: async ({}, use) => {
    // Unique-enough per test run; not guaranteed globally unique across parallel workers.
    const email = `e2e+${Date.now()}@aether.test`;
    await use(email);
  },

  authedPage: async ({ page, testEmail }, use) => {
    await signUpAndLogin(page, testEmail);
    await use(page);
  },
});

export { expect } from "@playwright/test";

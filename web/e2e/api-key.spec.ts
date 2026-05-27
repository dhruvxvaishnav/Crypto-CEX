/**
 * E2E-04 — Public REST: signup, generate API key, send signed POST /orders,
 *           receive 201.
 * PRD §20.4 E2E-04
 *
 * This spec is API-level (no browser UI beyond sign-up). It exercises the
 * HMAC-SHA256 request signing scheme from `services/api/src/extractors/hmac.rs`.
 * Payload format: `{timestampMs}\n{METHOD}\n{/path?query}\n{body}`
 */

import { expect, test } from "@playwright/test";
import { API_BASE, buildHmacSignature, signUpAndLogin, getAccessToken } from "./fixtures";

test.describe("E2E-04: HMAC-signed API key order", () => {
  test(
    "sign up, create API key, sign POST /orders with HMAC, receive 201",
    async ({ page, request }) => {
      const email = `e2e-apikey+${Date.now()}@aether.test`;

      // ── 1. Sign up via the browser (sets the session cookie) ───────────────
      await signUpAndLogin(page, email);
      const accessToken = await getAccessToken(page);

      // ── 2. Fund the account so the order can be placed ────────────────────
      await request.post(`${API_BASE}/wallet/faucet`, {
        data: { asset: "USDT", amount: "1000" },
        headers: { Authorization: `Bearer ${accessToken}` },
      });

      // ── 3. Create an API key (read + trade permissions) ────────────────────
      const createKeyResp = await request.post(`${API_BASE}/account/api-keys`, {
        data: { label: "e2e-test-key", permissions: ["read", "trade"] },
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      expect(createKeyResp.status()).toBe(201);

      const { id: keyId, secret: keySecret } = (await createKeyResp.json()) as {
        id: string;
        secret: string;
      };
      expect(keyId).toBeTruthy();
      expect(keySecret).toBeTruthy();

      // ── 4. Build a signed limit buy order (price well below market) ────────
      const method = "POST";
      const path = "/api/v1/orders";
      const body = JSON.stringify({
        symbol: "BTCUSDT",
        side: "buy",
        type: "limit",
        price: "1.00",
        quantity: "0.001",
      });
      const timestampMs = Date.now().toString();
      const signature = buildHmacSignature(keySecret, timestampMs, method, path, body);

      // ── 5. Send the signed request directly to the Rust API ───────────────
      const orderResp = await request.post(`${API_BASE}/orders`, {
        data: body,
        headers: {
          "Content-Type": "application/json",
          "X-AETHER-KEY": keyId,
          "X-AETHER-TS": timestampMs,
          "X-AETHER-SIGN": signature,
        },
      });

      // ── 6. Assert 201 Created ─────────────────────────────────────────────
      expect(orderResp.status()).toBe(201);

      const order = (await orderResp.json()) as { id: string; status: string };
      expect(order.id).toBeTruthy();
      expect(["open", "pending"]).toContain(order.status);
    },
  );
});

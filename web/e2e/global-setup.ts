/**
 * Playwright global setup — runs once before all specs.
 *
 * Checks that the API backend is reachable. When it isn't (e.g. plain CI
 * without the Rust stack), specs that hit the backend will fail with a
 * network error rather than a timeout, which gives a cleaner signal.
 *
 * To run against the deployed environment:
 *   PLAYWRIGHT_BASE_URL=https://aether-web.vercel.app \
 *   API_BASE_URL=https://aether-api.fly.dev/api/v1 \
 *   pnpm e2e
 */

const API_BASE = process.env.API_BASE_URL ?? "http://localhost:8080/api/v1";

export default async function globalSetup(): Promise<void> {
  try {
    const res = await fetch(`${API_BASE}/health`, { signal: AbortSignal.timeout(4_000) });
    if (res.ok) {
      console.log(`[e2e] API backend reachable at ${API_BASE}`);
    } else {
      console.warn(`[e2e] API /health returned ${res.status} — some specs may fail`);
    }
  } catch {
    console.warn(
      `[e2e] API backend not reachable at ${API_BASE}.\n` +
        "      Start the full stack (make dev) or set API_BASE_URL=<deployed url>.\n" +
        "      Only home.spec.ts will pass without a running backend.",
    );
  }
}

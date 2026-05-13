import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api-client", () => ({
  setAccessToken: vi.fn(),
  setRefreshFn: vi.fn(),
}));

describe("auth.store", () => {
  beforeEach(() => {
    vi.resetModules();
    // Clear any persisted state between tests.
    const storage = {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
    };
    vi.stubGlobal("window", { sessionStorage: storage });
  });

  it("setAuth stores user and token", async () => {
    const { useAuthStore } = await import("@/stores/auth.store");
    const { setAccessToken } = await import("@/lib/api-client");

    const user = { id: "u1", email: "test@example.com", totpEnabled: false };
    useAuthStore.getState().setAuth(user, "access-abc");

    const state = useAuthStore.getState();
    expect(state.user).toEqual(user);
    expect(state.accessToken).toBe("access-abc");
    expect(setAccessToken).toHaveBeenCalledWith("access-abc");
  });

  it("clearAuth resets to null", async () => {
    const { useAuthStore } = await import("@/stores/auth.store");

    const user = { id: "u2", email: "b@example.com", totpEnabled: false };
    useAuthStore.getState().setAuth(user, "tok");
    useAuthStore.getState().clearAuth();

    const state = useAuthStore.getState();
    expect(state.user).toBeNull();
    expect(state.accessToken).toBeNull();
  });
});

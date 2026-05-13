import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError, setAccessToken } from "@/lib/api-client";

const mockFetch = vi.fn<typeof fetch>();
vi.stubGlobal("fetch", mockFetch);

function makeOkResponse(body: unknown, status = 200) {
  return {
    ok: true,
    status,
    json: async () => body,
  } as Response;
}

function makeErrorResponse(body: unknown, status: number) {
  return {
    ok: false,
    status,
    json: async () => body,
  } as Response;
}

describe("api-client", () => {
  beforeEach(() => {
    mockFetch.mockReset();
    setAccessToken(null);
  });

  it("sends GET with Authorization header when token is set", async () => {
    setAccessToken("test-token");
    mockFetch.mockResolvedValueOnce(makeOkResponse({ id: "1" }));

    const { api } = await import("@/lib/api-client");
    await api.get("/markets");

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    expect(new Headers(init?.headers).get("Authorization")).toBe("Bearer test-token");
  });

  it("omits Authorization header when no token", async () => {
    mockFetch.mockResolvedValueOnce(makeOkResponse({}));

    const { api } = await import("@/lib/api-client");
    await api.get("/health");

    const [, init] = mockFetch.mock.calls[0] as [string, RequestInit];
    expect(new Headers(init?.headers).get("Authorization")).toBeNull();
  });

  it("throws ApiError with correct code on non-ok response", async () => {
    mockFetch.mockResolvedValueOnce(
      makeErrorResponse(
        { error: { code: "NOT_FOUND", message: "Not found" }, requestId: "req-1" },
        404,
      ),
    );

    const { api } = await import("@/lib/api-client");
    await expect(api.get("/orders/missing")).rejects.toMatchObject({
      code: "NOT_FOUND",
      status: 404,
      requestId: "req-1",
    });
  });

  it("sets code to UNKNOWN when error body is missing", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
      json: async () => {
        throw new Error("not json");
      },
    } as unknown as Response);

    const { api } = await import("@/lib/api-client");
    await expect(api.get("/broken")).rejects.toMatchObject({
      code: "UNKNOWN",
      status: 500,
    });
  });

  it("ApiError is instanceof Error", () => {
    const err = new ApiError("FOO", "msg", 400, "rid");
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("ApiError");
  });
});

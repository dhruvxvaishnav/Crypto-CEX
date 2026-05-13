import type { ApiErrorEnvelope } from "@/types/api.types";

export class ApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly status: number,
    public readonly requestId: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

// Resolved at module level; token injected per-request via setter.
let _accessToken: string | null = null;
let _refreshFn: (() => Promise<string | null>) | null = null;

export function setAccessToken(token: string | null): void {
  _accessToken = token;
}

export function setRefreshFn(fn: (() => Promise<string | null>) | null): void {
  _refreshFn = fn;
}

const BASE = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080/api/v1";

async function request<T>(
  path: string,
  init: RequestInit & { skipAuth?: boolean } = {},
): Promise<T> {
  const { skipAuth, ...fetchInit } = init;

  const headers = new Headers(fetchInit.headers);
  headers.set("Content-Type", "application/json");
  headers.set("Accept", "application/json");

  if (!skipAuth && _accessToken) {
    headers.set("Authorization", `Bearer ${_accessToken}`);
  }

  let res = await fetch(`${BASE}${path}`, { ...fetchInit, headers });

  // Attempt silent token refresh on 401.
  if (res.status === 401 && !skipAuth && _refreshFn) {
    const newToken = await _refreshFn();
    if (newToken) {
      headers.set("Authorization", `Bearer ${newToken}`);
      res = await fetch(`${BASE}${path}`, { ...fetchInit, headers });
    }
  }

  if (!res.ok) {
    let envelope: ApiErrorEnvelope | undefined;
    try {
      envelope = (await res.json()) as ApiErrorEnvelope;
    } catch {
      // Non-JSON error body — ignore.
    }
    throw new ApiError(
      envelope?.error.code ?? "UNKNOWN",
      envelope?.error.message ?? "Request failed",
      res.status,
      envelope?.requestId ?? "",
    );
  }

  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

export const api = {
  get: <T>(path: string, init?: RequestInit) => request<T>(path, { ...init, method: "GET" }),

  post: <T>(path: string, body: unknown, init?: RequestInit) =>
    request<T>(path, {
      ...init,
      method: "POST",
      body: JSON.stringify(body),
    }),

  delete: <T>(path: string, init?: RequestInit) => request<T>(path, { ...init, method: "DELETE" }),

  postPublic: <T>(path: string, body: unknown) =>
    request<T>(path, { method: "POST", body: JSON.stringify(body), skipAuth: true }),
};

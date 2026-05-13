"use client";

import { useRouter } from "next/navigation";
import { useCallback } from "react";

import { ApiError } from "@/lib/api-client";
import { useAuthStore } from "@/stores/auth.store";
import type { UserProfile } from "@/types/api.types";
import type { AuthUser } from "@/types/auth.types";

interface LoginResult {
  accessToken: string;
  expiresIn: number;
}

interface MfaResult {
  mfaRequired: true;
  mfaToken: string;
}

type AuthResult = LoginResult | MfaResult;

function isMfaResult(r: AuthResult): r is MfaResult {
  return "mfaRequired" in r;
}

async function proxyPost<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    const err = (await res.json()) as { error?: { code?: string; message?: string } };
    throw new ApiError(
      err.error?.code ?? "UNKNOWN",
      err.error?.message ?? "Request failed",
      res.status,
      "",
    );
  }

  return (await res.json()) as T;
}

async function fetchProfile(accessToken: string): Promise<AuthUser> {
  const res = await fetch(
    `${process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080/api/v1"}/account`,
    {
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
  if (!res.ok) {
    throw new ApiError("UNAUTHENTICATED", "Failed to load profile", res.status, "");
  }
  const data = (await res.json()) as UserProfile;
  return { id: data.id, email: data.email, totpEnabled: data.totpEnabled };
}

/** React hook that returns callable auth actions with router + store wired up. */
export function useAuthActions() {
  const { setAuth } = useAuthStore();
  const router = useRouter();

  const finishAuth = useCallback(
    async (accessToken: string) => {
      const user = await fetchProfile(accessToken);
      setAuth(user, accessToken);
      router.push("/trade/BTCUSDT");
    },
    [setAuth, router],
  );

  const login = useCallback(
    async (email: string, password: string): Promise<MfaResult | undefined> => {
      const result = await proxyPost<AuthResult>("/api/auth/login", { email, password });
      if (isMfaResult(result)) return result;
      await finishAuth(result.accessToken);
      return undefined;
    },
    [finishAuth],
  );

  const signup = useCallback(
    async (email: string, password: string): Promise<void> => {
      const result = await proxyPost<LoginResult>("/api/auth/signup", { email, password });
      await finishAuth(result.accessToken);
    },
    [finishAuth],
  );

  const verify2fa = useCallback(
    async (mfaToken: string, code: string): Promise<void> => {
      // POST to backend directly; 2FA verify returns new token pair via the backend.
      const result = await proxyPost<LoginResult>("/api/auth/login", { mfaToken, totpCode: code });
      await finishAuth(result.accessToken);
    },
    [finishAuth],
  );

  return { login, signup, verify2fa };
}

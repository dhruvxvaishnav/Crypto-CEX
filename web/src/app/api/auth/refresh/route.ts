import { cookies } from "next/headers";
import { NextResponse } from "next/server";

import type { AuthTokens } from "@/types/auth.types";

const API_BASE = process.env.API_BASE_URL ?? "http://localhost:8080/api/v1";

export async function POST(): Promise<NextResponse> {
  const cookieStore = await cookies();
  const refreshToken = cookieStore.get("aether_refresh")?.value;

  if (!refreshToken) {
    return NextResponse.json(
      { error: { code: "UNAUTHENTICATED", message: "No refresh token" } },
      { status: 401 },
    );
  }

  const upstream = await fetch(`${API_BASE}/auth/refresh`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ refreshToken }),
  });

  if (!upstream.ok) {
    // Revoke the stale refresh cookie.
    cookieStore.delete("aether_refresh");
    const err = (await upstream.json()) as unknown;
    return NextResponse.json(err, { status: upstream.status });
  }

  const data = (await upstream.json()) as AuthTokens;

  cookieStore.set("aether_refresh", data.refreshToken, {
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "lax",
    maxAge: 60 * 60 * 24 * 7,
    path: "/",
  });

  return NextResponse.json({ accessToken: data.accessToken, expiresIn: data.expiresIn });
}

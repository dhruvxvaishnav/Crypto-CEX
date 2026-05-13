import { cookies } from "next/headers";
import { NextResponse } from "next/server";

import type { AuthTokens, LoginResponse } from "@/types/auth.types";
import { isMfaRequired } from "@/types/auth.types";

const API_BASE = process.env.API_BASE_URL ?? "http://localhost:8080/api/v1";

export async function POST(request: Request): Promise<NextResponse> {
  const body = (await request.json()) as unknown;

  const upstream = await fetch(`${API_BASE}/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!upstream.ok) {
    const err = (await upstream.json()) as unknown;
    return NextResponse.json(err, { status: upstream.status });
  }

  const data = (await upstream.json()) as LoginResponse;

  if (!isMfaRequired(data)) {
    const tokens = data as AuthTokens;
    const cookieStore = await cookies();
    cookieStore.set("aether_refresh", tokens.refreshToken, {
      httpOnly: true,
      secure: process.env.NODE_ENV === "production",
      sameSite: "lax",
      // 7 days
      maxAge: 60 * 60 * 24 * 7,
      path: "/",
    });
    // Return access token + user-info subset to client (no refresh in body).
    return NextResponse.json({ accessToken: tokens.accessToken, expiresIn: tokens.expiresIn });
  }

  // MFA required — return mfa_token for next step.
  return NextResponse.json(data);
}

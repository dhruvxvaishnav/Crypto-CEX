import { cookies } from "next/headers";
import { NextResponse } from "next/server";

import type { AuthTokens } from "@/types/auth.types";

const API_BASE = process.env.API_BASE_URL ?? "http://localhost:8080/api/v1";

export async function POST(request: Request): Promise<NextResponse> {
  const body = (await request.json()) as unknown;

  const upstream = await fetch(`${API_BASE}/auth/signup`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  if (!upstream.ok) {
    const err = (await upstream.json()) as unknown;
    return NextResponse.json(err, { status: upstream.status });
  }

  const data = (await upstream.json()) as AuthTokens;

  const cookieStore = await cookies();
  cookieStore.set("aether_refresh", data.refreshToken, {
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "lax",
    maxAge: 60 * 60 * 24 * 7,
    path: "/",
  });

  return NextResponse.json({ accessToken: data.accessToken, expiresIn: data.expiresIn });
}

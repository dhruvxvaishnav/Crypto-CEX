import type { NextRequest } from "next/server";
import { NextResponse } from "next/server";

const PUBLIC_PATHS = new Set([
  "/",
  "/markets",
  "/proof-of-reserves",
  "/login",
  "/signup",
  "/2fa",
  "/forgot",
]);

const AUTH_ONLY_PREFIXES = ["/trade", "/portfolio", "/wallet", "/account", "/admin"];

export function proxy(request: NextRequest): NextResponse {
  const { pathname } = request.nextUrl;

  // Allow public paths and API routes unconditionally.
  if (
    PUBLIC_PATHS.has(pathname) ||
    pathname.startsWith("/api/") ||
    pathname.startsWith("/_next/") ||
    pathname.startsWith("/legal/")
  ) {
    return NextResponse.next();
  }

  const requiresAuth = AUTH_ONLY_PREFIXES.some((prefix) => pathname.startsWith(prefix));
  if (!requiresAuth) return NextResponse.next();

  // httpOnly refresh token is the session indicator.
  const hasSession = request.cookies.has("aether_refresh");
  if (!hasSession) {
    const loginUrl = new URL("/login", request.url);
    loginUrl.searchParams.set("next", pathname);
    return NextResponse.redirect(loginUrl);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|.*\\.svg).*)"],
};

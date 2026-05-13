"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { ThemeProvider } from "next-themes";
import { type ReactNode, useEffect, useRef } from "react";
import { api, setRefreshFn } from "@/lib/api-client";
import { syncTokenToClient, useAuthStore } from "@/stores/auth.store";
import type { AuthTokens } from "@/types/auth.types";

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        retry: 1,
        refetchOnWindowFocus: false,
      },
    },
  });
}

/** Singleton QueryClient on the browser; new instance per SSR request. */
let browserQueryClient: QueryClient | undefined;

function getQueryClient() {
  if (typeof window === "undefined") return makeQueryClient();
  browserQueryClient ??= makeQueryClient();
  return browserQueryClient;
}

// ── Token sync ────────────────────────────────────────────────────────────────

function TokenSync() {
  const { accessToken, setAuth, clearAuth, user } = useAuthStore();
  const initialised = useRef(false);

  useEffect(() => {
    if (initialised.current) return;
    initialised.current = true;

    // Rehydrate api-client from persisted store.
    syncTokenToClient(accessToken);

    // Wire refresh function so api-client can silently refresh tokens.
    setRefreshFn(async () => {
      try {
        const tokens = await api.postPublic<AuthTokens>("/auth/refresh", {
          refreshToken:
            document.cookie
              .split("; ")
              .find((c) => c.startsWith("aether_refresh="))
              ?.split("=")?.[1] ?? "",
        });
        if (user) setAuth(user, tokens.accessToken);
        return tokens.accessToken;
      } catch {
        clearAuth();
        return null;
      }
    });
  }, [accessToken, setAuth, clearAuth, user]);

  return null;
}

// ── Providers ─────────────────────────────────────────────────────────────────

interface ProvidersProps {
  children: ReactNode;
}

export function Providers({ children }: ProvidersProps) {
  const queryClient = getQueryClient();

  return (
    <ThemeProvider attribute="class" defaultTheme="dark" enableSystem disableTransitionOnChange>
      <QueryClientProvider client={queryClient}>
        <TokenSync />
        {children}
        {process.env.NODE_ENV === "development" && <ReactQueryDevtools initialIsOpen={false} />}
      </QueryClientProvider>
    </ThemeProvider>
  );
}

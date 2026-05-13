"use client";

import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { setAccessToken } from "@/lib/api-client";
import type { AuthUser } from "@/types/auth.types";

interface AuthStore {
  user: AuthUser | null;
  accessToken: string | null;
  setAuth: (user: AuthUser, accessToken: string) => void;
  clearAuth: () => void;
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      user: null,
      accessToken: null,

      setAuth(user, accessToken) {
        setAccessToken(accessToken);
        set({ user, accessToken });
      },

      clearAuth() {
        setAccessToken(null);
        set({ user: null, accessToken: null });
      },
    }),
    {
      name: "aether-auth",
      // sessionStorage so token clears on tab close.
      storage: createJSONStorage(() =>
        typeof window !== "undefined"
          ? window.sessionStorage
          : { getItem: () => null, setItem: () => {}, removeItem: () => {} },
      ),
      // Only persist the accessToken; user is re-fetched from it.
      partialize: (state) => ({ accessToken: state.accessToken, user: state.user }),
    },
  ),
);

/** Rehydrate the api-client token from persisted store on app load. */
export function syncTokenToClient(token: string | null): void {
  setAccessToken(token);
}

import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";

export const metadata: Metadata = {
  title: "Auth",
};

export default function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col bg-zinc-950">
      {/* Minimal top bar */}
      <header className="flex h-12 items-center border-b border-zinc-800 px-6">
        <Link href="/" className="flex items-center gap-2 font-bold text-zinc-100">
          <span className="text-emerald-400">⬡</span>
          <span className="text-sm tracking-wide">AETHER</span>
        </Link>
      </header>

      {/* Centred auth card */}
      <main id="main-content" className="flex flex-1 items-center justify-center px-4 py-12">
        <div className="w-full max-w-sm">{children}</div>
      </main>

      <footer className="py-4 text-center text-xs text-zinc-600">
        Aether is a portfolio demo. No real funds.{" "}
        <Link href="/legal/terms" className="underline hover:text-zinc-400">
          Terms
        </Link>
      </footer>
    </div>
  );
}

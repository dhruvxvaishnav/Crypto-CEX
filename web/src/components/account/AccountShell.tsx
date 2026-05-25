"use client";

import { KeyRound, Landmark, LayoutDashboard, Wallet } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import { TopBar } from "@/components/layout/TopBar";

interface AccountShellProps {
  children: ReactNode;
}

const NAV_ITEMS = [
  { href: "/portfolio", label: "Portfolio", icon: LayoutDashboard },
  { href: "/wallet", label: "Wallet", icon: Wallet },
  { href: "/account", label: "Account", icon: KeyRound },
];

export function AccountShell({ children }: AccountShellProps) {
  const pathname = usePathname();

  return (
    <div className="flex min-h-screen flex-col bg-zinc-950 text-zinc-100">
      <TopBar />
      <main id="main-content" className="flex flex-1">
        <aside className="hidden w-56 shrink-0 border-r border-zinc-800 bg-zinc-950/95 p-3 md:block">
          <nav aria-label="Account sections" className="flex flex-col gap-1">
            {NAV_ITEMS.map((item) => {
              const Icon = item.icon;
              const active = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={[
                    "flex h-9 items-center gap-2 rounded-md px-3 text-sm transition-colors",
                    active
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100",
                  ].join(" ")}
                >
                  <Icon className="h-4 w-4" aria-hidden />
                  {item.label}
                </Link>
              );
            })}
          </nav>
          <div className="mt-4 rounded-md border border-emerald-900/70 bg-emerald-950/20 p-3">
            <div className="mb-2 flex items-center gap-2 text-xs font-medium text-emerald-300">
              <Landmark className="h-3.5 w-3.5" aria-hidden />
              Demo exchange
            </div>
            <p className="text-xs leading-5 text-zinc-500">
              Aether is a portfolio demonstration. It does not custody real funds.
            </p>
          </div>
        </aside>
        <section className="min-w-0 flex-1 overflow-auto">
          <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8">
            {children}
            <p className="border-t border-zinc-900 pt-4 text-xs text-zinc-600 md:hidden">
              Aether is a portfolio demonstration. It does not custody real funds.
            </p>
          </div>
        </section>
      </main>
    </div>
  );
}

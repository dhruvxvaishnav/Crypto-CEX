"use client";

import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { LayoutDashboard, LogOut, Settings, User, Wallet } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";

import { useAuthStore } from "@/stores/auth.store";
import { MarketSelector } from "./MarketSelector";
import type { TopBarProps } from "./TopBar.types";

export function TopBar({ activeSymbol, marketStats, markets = [], onMarketSelect }: TopBarProps) {
  const router = useRouter();
  const { user, clearAuth } = useAuthStore();

  const pct = marketStats?.priceChangePct24h ?? null;
  const pctPositive = pct !== null && !pct.startsWith("-");

  function handleLogout() {
    clearAuth();
    router.push("/login");
  }

  function handleMarketSelect(symbol: string) {
    onMarketSelect?.(symbol);
    router.push(`/trade/${symbol}`);
  }

  return (
    <header className="flex h-12 shrink-0 items-center gap-4 border-b border-zinc-800 bg-zinc-950 px-4">
      {/* Logo */}
      <Link
        href="/"
        className="flex items-center gap-2 font-bold text-zinc-100 hover:text-emerald-400 transition-colors mr-2"
      >
        <span className="text-emerald-400">⬡</span>
        <span className="text-sm tracking-wide">AETHER</span>
      </Link>

      {/* Market selector */}
      {activeSymbol !== undefined && (
        <MarketSelector
          current={activeSymbol ?? "Select market"}
          markets={markets}
          onSelect={handleMarketSelect}
        />
      )}

      {/* 24h stats — shown when on a trade page */}
      {activeSymbol && marketStats && (
        <div className="hidden items-center gap-5 text-xs md:flex">
          {marketStats.lastPrice && (
            <div className="flex flex-col">
              <span className="text-zinc-500 text-[10px]">Last</span>
              <span className="font-medium text-zinc-100">{marketStats.lastPrice}</span>
            </div>
          )}
          {pct !== null && (
            <div className="flex flex-col">
              <span className="text-zinc-500 text-[10px]">24h %</span>
              <span className={`font-medium ${pctPositive ? "text-emerald-400" : "text-rose-400"}`}>
                {pctPositive ? "+" : ""}
                {pct}%
              </span>
            </div>
          )}
          {marketStats.volume24h && (
            <div className="flex flex-col">
              <span className="text-zinc-500 text-[10px]">24h Vol</span>
              <span className="font-medium text-zinc-300">{marketStats.volume24h}</span>
            </div>
          )}
        </div>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Nav links */}
      <nav aria-label="Primary" className="hidden items-center gap-1 text-xs text-zinc-400 md:flex">
        <Link
          href="/markets"
          className="rounded px-2 py-1.5 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        >
          Markets
        </Link>
        <Link
          href="/trade/BTCUSDT"
          className="rounded px-2 py-1.5 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        >
          Trade
        </Link>
        <Link
          href="/portfolio"
          className="rounded px-2 py-1.5 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
        >
          Portfolio
        </Link>
      </nav>

      {/* Account menu */}
      {user ? (
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <button
              type="button"
              className="flex h-7 w-7 items-center justify-center rounded-full bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500"
              aria-label="Account menu"
            >
              <User className="h-4 w-4" aria-hidden />
            </button>
          </DropdownMenu.Trigger>

          <DropdownMenu.Portal>
            <DropdownMenu.Content
              sideOffset={8}
              align="end"
              className="z-50 w-52 rounded-lg border border-zinc-700 bg-zinc-900 py-1 shadow-xl shadow-black/50 animate-in fade-in-0 zoom-in-95"
            >
              <div className="px-3 py-2 border-b border-zinc-800">
                <p className="text-xs font-medium text-zinc-100 truncate">{user.email}</p>
              </div>

              <DropdownMenu.Item asChild>
                <Link
                  href="/portfolio"
                  className="flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-zinc-300 outline-none hover:bg-zinc-800 hover:text-zinc-100 focus:bg-zinc-800"
                >
                  <LayoutDashboard className="h-4 w-4" aria-hidden />
                  Portfolio
                </Link>
              </DropdownMenu.Item>

              <DropdownMenu.Item asChild>
                <Link
                  href="/wallet"
                  className="flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-zinc-300 outline-none hover:bg-zinc-800 hover:text-zinc-100 focus:bg-zinc-800"
                >
                  <Wallet className="h-4 w-4" aria-hidden />
                  Wallet
                </Link>
              </DropdownMenu.Item>

              <DropdownMenu.Item asChild>
                <Link
                  href="/account"
                  className="flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-zinc-300 outline-none hover:bg-zinc-800 hover:text-zinc-100 focus:bg-zinc-800"
                >
                  <Settings className="h-4 w-4" aria-hidden />
                  Account
                </Link>
              </DropdownMenu.Item>

              <DropdownMenu.Separator className="my-1 h-px bg-zinc-800" />

              <DropdownMenu.Item
                className="flex cursor-pointer items-center gap-2 px-3 py-2 text-sm text-rose-400 outline-none hover:bg-zinc-800 hover:text-rose-300 focus:bg-zinc-800"
                onSelect={handleLogout}
              >
                <LogOut className="h-4 w-4" aria-hidden />
                Sign out
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      ) : (
        <div className="flex items-center gap-2">
          <Link
            href="/login"
            className="rounded px-3 py-1.5 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
          >
            Sign in
          </Link>
          <Link
            href="/signup"
            className="rounded bg-emerald-500 px-3 py-1.5 text-xs font-medium text-zinc-950 hover:bg-emerald-400 transition-colors"
          >
            Get started
          </Link>
        </div>
      )}
    </header>
  );
}

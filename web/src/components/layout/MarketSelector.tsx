"use client";

import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { ChevronDown, Search } from "lucide-react";
import { useState } from "react";

import type { MarketSelectorProps } from "./MarketSelector.types";

export function MarketSelector({ current, markets, onSelect }: MarketSelectorProps) {
  const [query, setQuery] = useState("");

  const filtered = query
    ? markets.filter((m) => m.symbol.toLowerCase().includes(query.toLowerCase()))
    : markets;

  return (
    <DropdownMenu.Root onOpenChange={() => setQuery("")}>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1.5 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-sm font-medium text-zinc-100 hover:border-zinc-600 hover:bg-zinc-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-500"
          aria-label="Select market"
        >
          {current}
          <ChevronDown className="h-3.5 w-3.5 text-zinc-400" aria-hidden />
        </button>
      </DropdownMenu.Trigger>

      <DropdownMenu.Portal>
        <DropdownMenu.Content
          sideOffset={6}
          align="start"
          className="z-50 w-64 rounded-lg border border-zinc-700 bg-zinc-900 shadow-xl shadow-black/50 animate-in fade-in-0 zoom-in-95"
        >
          {/* Search */}
          <div className="flex items-center gap-2 border-b border-zinc-800 px-3 py-2">
            <Search className="h-3.5 w-3.5 text-zinc-500 shrink-0" aria-hidden />
            <input
              className="flex-1 bg-transparent text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none"
              placeholder="Search markets…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label="Search markets"
            />
          </div>

          {/* Market list */}
          <div className="max-h-72 overflow-y-auto py-1">
            {filtered.length === 0 ? (
              <p className="px-3 py-4 text-center text-xs text-zinc-500">No markets found</p>
            ) : (
              filtered.map((market) => {
                const pct = market.priceChangePct24h;
                const isPositive = pct !== null && !pct.startsWith("-");
                const pctColor =
                  pct === null
                    ? "text-zinc-500"
                    : isPositive
                      ? "text-emerald-400"
                      : "text-rose-400";

                return (
                  <DropdownMenu.Item
                    key={market.symbol}
                    onSelect={() => onSelect(market.symbol)}
                    className="flex cursor-pointer items-center justify-between px-3 py-2 text-sm outline-none hover:bg-zinc-800 focus:bg-zinc-800 data-[highlighted]:bg-zinc-800"
                  >
                    <span className="font-medium text-zinc-100">{market.symbol}</span>
                    <div className="flex flex-col items-end">
                      {market.lastPrice && (
                        <span className="text-xs text-zinc-200">{market.lastPrice}</span>
                      )}
                      {pct !== null && (
                        <span className={`text-xs ${pctColor}`}>
                          {isPositive ? "+" : ""}
                          {pct}%
                        </span>
                      )}
                    </div>
                  </DropdownMenu.Item>
                );
              })
            )}
          </div>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

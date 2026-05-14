"use client";

import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { getRecentTrades, isTrade } from "@/lib/market-data";
import type { Trade } from "@/types/api.types";

import type { RecentTradesProps } from "./RecentTrades.types";

const MAX_TRADES = 80;
const TIME_FORMATTER = new Intl.DateTimeFormat("en", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
  timeZone: "UTC",
});

export function RecentTrades({ symbol, ws }: RecentTradesProps) {
  const [trades, setTrades] = useState<Trade[]>([]);
  const tradesQuery = useQuery({
    queryKey: ["trades", symbol],
    queryFn: () => getRecentTrades(symbol),
  });

  useEffect(() => {
    if (tradesQuery.data) setTrades(tradesQuery.data);
  }, [tradesQuery.data]);

  useEffect(() => {
    return ws.subscribe(`trade.${symbol}`, (payload) => {
      if (!isTrade(payload)) return;
      setTrades((current) =>
        [payload, ...current.filter((trade) => trade.id !== payload.id)].slice(0, MAX_TRADES),
      );
    });
  }, [symbol, ws.subscribe]);

  return (
    <section className="flex min-h-0 flex-col border-t border-zinc-800 bg-zinc-950 lg:border-t-0">
      <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
        <h2 className="text-sm font-semibold text-zinc-100">Recent trades</h2>
        <span className="text-[11px] text-zinc-500">UTC</span>
      </div>
      <AsyncBoundary
        isLoading={tradesQuery.isLoading && trades.length === 0}
        isEmpty={!tradesQuery.isLoading && trades.length === 0}
      >
        <div className="grid grid-cols-[1fr_1fr_1fr] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
          <span>Price</span>
          <span className="text-right">Qty</span>
          <span className="text-right">Time</span>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {trades.map((trade) => {
            const sideClass = trade.side === "buy" ? "text-emerald-400" : "text-rose-400";
            return (
              <div
                key={trade.id}
                className="grid grid-cols-[1fr_1fr_1fr] px-3 text-[11px] leading-6 hover:bg-zinc-900/70"
              >
                <span className={`font-medium ${sideClass}`}>{trade.price}</span>
                <span className="text-right text-zinc-300">{trade.qty}</span>
                <span className="text-right text-zinc-500">{formatTradeTime(trade.ts)}</span>
              </div>
            );
          })}
        </div>
      </AsyncBoundary>
    </section>
  );
}

function formatTradeTime(ts: string): string {
  const date = new Date(ts);
  if (Number.isNaN(date.valueOf())) return "--";
  return TIME_FORMATTER.format(date);
}

"use client";

import { useQuery } from "@tanstack/react-query";
import { Wifi, WifiOff } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { getOrderBook, isOrderBookDelta } from "@/lib/market-data";

import type { OrderBookProps } from "./OrderBook.types";
import type { BookSideRow, OrderBookState } from "./order-book-model";
import { mergeBookDelta, snapshotToState } from "./order-book-model";

const ROW_HEIGHT_PX = 24;
const VISIBLE_ROWS = 16;
const OVERSCAN_ROWS = 4;

export function OrderBook({ symbol, ws }: OrderBookProps) {
  const [book, setBook] = useState<OrderBookState | null>(null);
  const snapshotQuery = useQuery({
    queryKey: ["orderbook", symbol],
    queryFn: () => getOrderBook(symbol),
    staleTime: 0,
  });

  useEffect(() => {
    if (snapshotQuery.data) {
      setBook(snapshotToState(snapshotQuery.data));
    }
  }, [snapshotQuery.data]);

  useEffect(() => {
    return ws.subscribe(`book.${symbol}.diff`, (payload) => {
      if (!isOrderBookDelta(payload)) return;
      setBook((current) => (current ? mergeBookDelta(current, payload) : snapshotToState(payload)));
    });
  }, [symbol, ws.subscribe]);

  const spread = useMemo(() => {
    const bestBid = book?.bids[0]?.price;
    const bestAsk = book?.asks[0]?.price;
    if (!bestBid || !bestAsk) return null;
    const spreadValue = Number(bestAsk) - Number(bestBid);
    if (!Number.isFinite(spreadValue) || spreadValue < 0) return "0";
    return spreadValue.toLocaleString(undefined, { maximumFractionDigits: 8 });
  }, [book]);

  return (
    <section className="flex min-h-0 flex-col border-r border-zinc-800 bg-zinc-950">
      <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
        <div>
          <h2 className="text-sm font-semibold text-zinc-100">Order book</h2>
          {spread && <p className="text-[11px] text-zinc-500">Spread {spread}</p>}
        </div>
        <div className="flex items-center gap-1.5 text-[11px] text-zinc-500">
          {ws.status === "connected" ? (
            <Wifi className="h-3.5 w-3.5 text-emerald-400" aria-hidden />
          ) : (
            <WifiOff className="h-3.5 w-3.5 text-amber-400" aria-hidden />
          )}
          {ws.status}
        </div>
      </div>

      <AsyncBoundary
        isLoading={snapshotQuery.isLoading && book === null}
        isEmpty={book !== null && book.bids.length === 0 && book.asks.length === 0}
      >
        <div className="grid grid-cols-[1fr_1fr_1fr] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
          <span>Price</span>
          <span className="text-right">Qty</span>
          <span className="text-right">Total</span>
        </div>
        <VirtualBookSide rows={book?.asks ?? []} side="asks" />
        <div className="border-y border-zinc-800 bg-zinc-900/60 px-3 py-2 text-center text-xs font-medium text-zinc-300">
          {book?.asks[0]?.price ?? "--"} / {book?.bids[0]?.price ?? "--"}
        </div>
        <VirtualBookSide rows={book?.bids ?? []} side="bids" />
      </AsyncBoundary>
    </section>
  );
}

function VirtualBookSide({ rows, side }: { rows: BookSideRow[]; side: "asks" | "bids" }) {
  const [scrollTop, setScrollTop] = useState(0);
  const totalHeight = rows.length * ROW_HEIGHT_PX;
  const startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT_PX) - OVERSCAN_ROWS);
  const endIndex = Math.min(rows.length, startIndex + VISIBLE_ROWS + OVERSCAN_ROWS * 2);
  const visibleRows = rows.slice(startIndex, endIndex);
  const topOffset = startIndex * ROW_HEIGHT_PX;
  const sideClass = side === "bids" ? "text-emerald-400" : "text-rose-400";
  const fillClass = side === "bids" ? "bg-emerald-500/10" : "bg-rose-500/10";

  return (
    <div
      className="h-96 overflow-y-auto"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div style={{ height: totalHeight, position: "relative" }}>
        <div style={{ transform: `translateY(${topOffset}px)` }}>
          {visibleRows.map((row) => (
            <div
              key={row.price}
              className="relative grid grid-cols-[1fr_1fr_1fr] px-3 text-[11px] leading-6"
              style={{ height: ROW_HEIGHT_PX }}
            >
              <span
                className={`absolute inset-y-0 right-0 ${fillClass}`}
                style={{ width: `${row.depthPct}%` }}
                aria-hidden
              />
              <span className={`relative font-medium ${sideClass}`}>{row.price}</span>
              <span className="relative text-right text-zinc-300">{row.quantity}</span>
              <span className="relative text-right text-zinc-500">{row.total}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

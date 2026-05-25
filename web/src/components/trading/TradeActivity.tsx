"use client";

import * as Tabs from "@radix-ui/react-tabs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { Button } from "@/components/ui/Button";
import { cancelOrder, getOrders } from "@/lib/trading-api";
import type { Order, UserFill } from "@/types/api.types";

import type { FillsTableProps, OrdersTableProps, TradeActivityProps } from "./TradeActivity.types";

const OPEN_STATUSES = new Set<Order["status"]>(["pending", "new", "partial"]);
const FINAL_STATUSES = new Set<Order["status"]>(["filled", "canceled", "rejected"]);

const TIME_FORMATTER = new Intl.DateTimeFormat("en", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
  timeZone: "UTC",
});

export function TradeActivity({ symbol, ws, latestOrder }: TradeActivityProps) {
  const queryClient = useQueryClient();
  const [fills, setFills] = useState<UserFill[]>([]);
  const ordersQuery = useQuery({
    queryKey: ["orders", symbol],
    queryFn: () => getOrders({ market: symbol }),
    staleTime: 0,
  });
  const orders = ordersQuery.data ?? [];
  const cancelMutation = useMutation({
    mutationFn: cancelOrder,
    onSuccess() {
      void queryClient.invalidateQueries({ queryKey: ["orders", symbol] });
      void queryClient.invalidateQueries({ queryKey: ["balances"] });
    },
  });

  useEffect(() => {
    if (!latestOrder) return;
    void queryClient.invalidateQueries({ queryKey: ["orders", symbol] });
  }, [latestOrder, queryClient, symbol]);

  useEffect(() => {
    return ws.subscribe("user.fills", (payload) => {
      if (!isUserFill(payload) || payload.symbol !== symbol) return;
      setFills((current) =>
        [payload, ...current.filter((fill) => fill.tradeId !== payload.tradeId)].slice(0, 80),
      );
    });
  }, [symbol, ws.subscribe]);

  const openOrders = useMemo(
    () => orders.filter((order) => OPEN_STATUSES.has(order.status)),
    [orders],
  );
  const orderHistory = useMemo(
    () => orders.filter((order) => FINAL_STATUSES.has(order.status)),
    [orders],
  );

  return (
    <section className="flex min-h-[260px] flex-col border-t border-zinc-800 bg-zinc-950">
      <Tabs.Root defaultValue="open" className="flex min-h-0 flex-1 flex-col">
        <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
          <Tabs.List className="flex gap-1" aria-label="Trading activity">
            <TabTrigger value="open">Open</TabTrigger>
            <TabTrigger value="orders">Orders</TabTrigger>
            <TabTrigger value="fills">Fills</TabTrigger>
          </Tabs.List>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="Refresh orders"
            title="Refresh"
            onClick={() => ordersQuery.refetch()}
          >
            <RefreshCw className="h-3.5 w-3.5" aria-hidden />
          </Button>
        </div>

        <Tabs.Content value="open" className="min-h-0 flex-1 overflow-hidden">
          <AsyncBoundary
            isLoading={ordersQuery.isLoading}
            isEmpty={!ordersQuery.isLoading && openOrders.length === 0}
          >
            <OrdersTable
              orders={openOrders}
              emptyLabel="No open orders"
              onCancel={(orderId) => cancelMutation.mutate(orderId)}
              pendingCancelId={cancelMutation.variables ?? null}
            />
          </AsyncBoundary>
        </Tabs.Content>

        <Tabs.Content value="orders" className="min-h-0 flex-1 overflow-hidden">
          <AsyncBoundary
            isLoading={ordersQuery.isLoading}
            isEmpty={!ordersQuery.isLoading && orderHistory.length === 0}
          >
            <OrdersTable orders={orderHistory} emptyLabel="No order history" />
          </AsyncBoundary>
        </Tabs.Content>

        <Tabs.Content value="fills" className="min-h-0 flex-1 overflow-hidden">
          <AsyncBoundary isEmpty={fills.length === 0}>
            <FillsTable fills={fills} />
          </AsyncBoundary>
        </Tabs.Content>
      </Tabs.Root>
    </section>
  );
}

function TabTrigger({ value, children }: { value: string; children: string }) {
  return (
    <Tabs.Trigger
      value={value}
      className="h-7 rounded px-3 text-xs font-medium text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-zinc-100 data-[state=active]:bg-zinc-800 data-[state=active]:text-zinc-100"
    >
      {children}
    </Tabs.Trigger>
  );
}

function OrdersTable({ orders, emptyLabel, onCancel, pendingCancelId }: OrdersTableProps) {
  if (orders.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-500">
        {emptyLabel}
      </div>
    );
  }

  return (
    <div className="min-h-0 overflow-auto">
      <div className="grid min-w-[760px] grid-cols-[120px_80px_90px_1fr_1fr_1fr_120px_48px] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
        <span>Time</span>
        <span>Side</span>
        <span>Type</span>
        <span className="text-right">Price</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Filled</span>
        <span className="text-right">Status</span>
        <span />
      </div>
      {orders.map((order) => (
        <div
          key={order.id}
          className="grid min-w-[760px] grid-cols-[120px_80px_90px_1fr_1fr_1fr_120px_48px] px-3 text-[11px] leading-8 hover:bg-zinc-900/70"
        >
          <span className="text-zinc-500">{formatTime(order.createdAt)}</span>
          <span className={order.side === "buy" ? "text-emerald-400" : "text-rose-400"}>
            {order.side.toUpperCase()}
          </span>
          <span className="text-zinc-300">{order.type.replace("_", " ")}</span>
          <span className="text-right text-zinc-300">{order.price ?? "--"}</span>
          <span className="text-right text-zinc-300">{order.quantity ?? order.quoteQuantity}</span>
          <span className="text-right text-zinc-500">{order.filledQuantity}</span>
          <span className="text-right text-zinc-400">{order.status}</span>
          <span className="flex justify-end">
            {onCancel && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                aria-label="Cancel order"
                title="Cancel order"
                isLoading={pendingCancelId === order.id}
                onClick={() => onCancel(order.id)}
              >
                <X className="h-3.5 w-3.5" aria-hidden />
              </Button>
            )}
          </span>
        </div>
      ))}
    </div>
  );
}

function FillsTable({ fills }: FillsTableProps) {
  return (
    <div className="min-h-0 overflow-auto">
      <div className="grid min-w-[560px] grid-cols-[120px_90px_1fr_1fr_1fr] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
        <span>Time</span>
        <span>Side</span>
        <span className="text-right">Price</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Trade</span>
      </div>
      {fills.map((fill) => (
        <div
          key={fill.tradeId}
          className="grid min-w-[560px] grid-cols-[120px_90px_1fr_1fr_1fr] px-3 text-[11px] leading-8 hover:bg-zinc-900/70"
        >
          <span className="text-zinc-500">{formatTime(fill.ts)}</span>
          <span className={fill.takerSide === "buy" ? "text-emerald-400" : "text-rose-400"}>
            {fill.takerSide.toUpperCase()}
          </span>
          <span className="text-right text-zinc-300">{fill.price}</span>
          <span className="text-right text-zinc-300">{fill.qty}</span>
          <span className="truncate text-right text-zinc-500">{fill.tradeId.slice(0, 8)}</span>
        </div>
      ))}
    </div>
  );
}

function isUserFill(value: unknown): value is UserFill {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.tradeId === "string" &&
    typeof record.symbol === "string" &&
    typeof record.takerOrderId === "string" &&
    typeof record.makerOrderId === "string" &&
    typeof record.price === "string" &&
    typeof record.qty === "string" &&
    (record.takerSide === "buy" || record.takerSide === "sell") &&
    typeof record.ts === "string"
  );
}

function formatTime(ts: string): string {
  const date = new Date(ts);
  if (Number.isNaN(date.valueOf())) return "--";
  return TIME_FORMATTER.format(date);
}

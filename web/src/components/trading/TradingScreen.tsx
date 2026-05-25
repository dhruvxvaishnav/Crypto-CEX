"use client";

import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { MarketSelectorOption } from "@/components/layout/MarketSelector.types";
import { TopBar } from "@/components/layout/TopBar";
import type { MarketStats } from "@/components/layout/TopBar.types";
import { getMarkets } from "@/lib/market-data";
import { useWebSocket } from "@/lib/ws-client";
import type { Market, Order, OrderSide } from "@/types/api.types";
import { matchTradingHotkey } from "./hotkeys";
import { OrderBook } from "./OrderBook";
import { OrderForm } from "./OrderForm";
import { PriceChart } from "./PriceChart";
import { RecentTrades } from "./RecentTrades";
import { TradeActivity } from "./TradeActivity";
import type { TradingScreenProps } from "./TradingScreen.types";

const ORDER_FORM_ID = "trading-order-form";

export function TradingScreen({ initialSymbol }: TradingScreenProps) {
  const symbol = initialSymbol.toUpperCase();
  const ws = useWebSocket();
  const [side, setSide] = useState<OrderSide>("buy");
  const [latestOrder, setLatestOrder] = useState<Order | null>(null);
  const marketsQuery = useQuery({
    queryKey: ["markets"],
    queryFn: getMarkets,
  });
  const markets = marketsQuery.data ?? [];
  const activeMarket = markets.find((market) => market.symbol === symbol) ?? null;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const action = matchTradingHotkey(event);
      if (!action) return;
      event.preventDefault();

      if (action === "buy") setSide("buy");
      if (action === "sell") setSide("sell");
      if (action === "focusOrderForm") document.getElementById("order-form-quantity")?.focus();
      if (action === "submitOrder") {
        const form = document.getElementById(ORDER_FORM_ID);
        if (form instanceof HTMLFormElement) form.requestSubmit();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="flex h-screen min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <TopBar
        activeSymbol={symbol}
        marketStats={activeMarket ? toMarketStats(activeMarket) : null}
        markets={markets.map(toSelectorOption)}
      />
      <main
        id="main-content"
        className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(360px,1fr)_minmax(260px,0.7fr)_minmax(360px,auto)_auto] overflow-hidden lg:grid-cols-[280px_minmax(0,1fr)_320px] lg:grid-rows-1"
      >
        <OrderBook symbol={symbol} ws={ws} />
        <div className="flex min-h-0 flex-col">
          <PriceChart symbol={symbol} ws={ws} />
          <TradeActivity symbol={symbol} ws={ws} latestOrder={latestOrder} />
        </div>
        <div className="flex min-h-0 flex-col border-l border-zinc-800">
          <OrderForm
            market={activeMarket}
            symbol={symbol}
            side={side}
            formId={ORDER_FORM_ID}
            onSideChange={setSide}
            onPlaced={setLatestOrder}
          />
          <RecentTrades symbol={symbol} ws={ws} />
        </div>
      </main>
    </div>
  );
}

function toSelectorOption(market: Market): MarketSelectorOption {
  return {
    symbol: market.symbol,
    baseAsset: market.baseAsset,
    quoteAsset: market.quoteAsset,
    lastPrice: market.lastPrice,
    priceChangePct24h: market.priceChangePct,
  };
}

function toMarketStats(market: Market): MarketStats {
  return {
    lastPrice: market.lastPrice,
    priceChange24h: null,
    priceChangePct24h: market.priceChangePct,
    volume24h: market.volume24h,
  };
}

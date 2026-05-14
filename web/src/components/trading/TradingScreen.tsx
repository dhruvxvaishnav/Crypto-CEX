"use client";

import { useQuery } from "@tanstack/react-query";

import type { MarketSelectorOption } from "@/components/layout/MarketSelector.types";
import { TopBar } from "@/components/layout/TopBar";
import type { MarketStats } from "@/components/layout/TopBar.types";
import { getMarkets } from "@/lib/market-data";
import { useWebSocket } from "@/lib/ws-client";
import type { Market } from "@/types/api.types";

import { OrderBook } from "./OrderBook";
import { PriceChart } from "./PriceChart";
import { RecentTrades } from "./RecentTrades";
import type { TradingScreenProps } from "./TradingScreen.types";

export function TradingScreen({ initialSymbol }: TradingScreenProps) {
  const symbol = initialSymbol.toUpperCase();
  const ws = useWebSocket();
  const marketsQuery = useQuery({
    queryKey: ["markets"],
    queryFn: getMarkets,
  });
  const markets = marketsQuery.data ?? [];
  const activeMarket = markets.find((market) => market.symbol === symbol) ?? null;

  return (
    <div className="flex h-screen min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <TopBar
        activeSymbol={symbol}
        marketStats={activeMarket ? toMarketStats(activeMarket) : null}
        markets={markets.map(toSelectorOption)}
      />
      <main
        id="main-content"
        className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(420px,1fr)_auto_auto] overflow-hidden lg:grid-cols-[280px_minmax(0,1fr)_300px] lg:grid-rows-1"
      >
        <OrderBook symbol={symbol} ws={ws} />
        <PriceChart symbol={symbol} ws={ws} />
        <RecentTrades symbol={symbol} ws={ws} />
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

import type { MarketSelectorOption } from "./MarketSelector.types";

export interface MarketStats {
  lastPrice: string | null;
  priceChange24h: string | null;
  priceChangePct24h: string | null;
  volume24h: string | null;
}

export interface TopBarProps {
  /** Currently active market symbol, e.g. "BTCUSDT". Null on non-trade pages. */
  activeSymbol?: string | null;
  marketStats?: MarketStats | null;
  markets?: MarketSelectorOption[];
  onMarketSelect?: (symbol: string) => void;
}

export interface MarketSelectorOption {
  symbol: string;
  baseAsset: string;
  quoteAsset: string;
  lastPrice: string | null;
  priceChangePct24h: string | null;
}

export interface MarketSelectorProps {
  current: string;
  markets: MarketSelectorOption[];
  onSelect: (symbol: string) => void;
}

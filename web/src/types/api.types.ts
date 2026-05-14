export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
  requestId: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  nextCursor: string | null;
}

export interface ApiDataResponse<T> {
  data: T;
}

export interface Market {
  id: string;
  symbol: string;
  baseAsset: string;
  quoteAsset: string;
  status: "active" | "halted" | "trading";
  tickSize: string;
  lotSize: string;
  minNotional: string;
  lastPrice: string | null;
  volume24h: string | null;
  high24h: string | null;
  low24h: string | null;
  priceChangePct: string | null;
  makerFeeBps: string;
  takerFeeBps: string;
}

export interface MarketTicker {
  symbol: string;
  lastPrice: string | null;
  open24h: string | null;
  high24h: string | null;
  low24h: string | null;
  volume24h: string | null;
  priceChangePct: string | null;
}

export type PriceLevelTuple = readonly [string, string];

export interface OrderBookSnapshot {
  seq: number;
  bids: PriceLevelTuple[];
  asks: PriceLevelTuple[];
}

export interface OrderBookDelta {
  seq: number;
  bids: PriceLevelTuple[];
  asks: PriceLevelTuple[];
}

export interface Trade {
  id: string;
  price: string;
  qty: string;
  side: "buy" | "sell";
  ts: string;
}

export interface Kline {
  ts: number;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
}

export interface Order {
  id: string;
  clientOrderId: string | null;
  symbol: string;
  side: "buy" | "sell";
  type: string;
  price: string | null;
  quantity: string | null;
  filledQty: string;
  status: string;
  createdAt: string;
}

export interface Balance {
  asset: string;
  free: string;
  locked: string;
  total: string;
}

export interface UserProfile {
  id: string;
  email: string;
  totpEnabled: boolean;
  createdAt: string;
}

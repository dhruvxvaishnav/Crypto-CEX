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

export interface Market {
  symbol: string;
  baseAsset: string;
  quoteAsset: string;
  status: "active" | "halted";
  tickSize: string;
  lotSize: string;
  minNotional: string;
  lastPrice: string | null;
  priceChange24h: string | null;
  priceChangePct24h: string | null;
  volume24h: string | null;
}

export interface OrderBookLevel {
  price: string;
  quantity: string;
}

export interface OrderBookSnapshot {
  symbol: string;
  seq: number;
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
}

export interface Trade {
  id: string;
  symbol: string;
  price: string;
  quantity: string;
  side: "buy" | "sell";
  executedAt: string;
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

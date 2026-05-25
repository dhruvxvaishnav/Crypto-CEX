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

export interface ApiListResponse<T> {
  data: T[];
  nextCursor: string | null;
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

export type OrderSide = "buy" | "sell";

export type OrderType =
  | "limit"
  | "market"
  | "ioc"
  | "fok"
  | "post_only"
  | "stop_limit"
  | "stop_market"
  | "oco";

export type OrderStatus = "pending" | "new" | "partial" | "filled" | "canceled" | "rejected";

export interface OrderFill {
  tradeId: string;
  price: string;
  quantity: string;
  side: OrderSide;
  ts: string;
}

export interface Order {
  id: string;
  clientOrderId: string | null;
  market: string;
  side: OrderSide;
  type: OrderType;
  status: OrderStatus;
  price: string | null;
  stopPrice: string | null;
  quantity: string | null;
  quoteQuantity: string | null;
  displayQuantity: string | null;
  filledQuantity: string;
  avgFillPrice: string | null;
  fills: OrderFill[];
  createdAt: string;
  updatedAt: string;
}

export interface PlaceOrderInput {
  clientOrderId: string;
  market: string;
  side: OrderSide;
  type: OrderType;
  price?: string;
  stopPrice?: string;
  quantity?: string;
  quoteQuantity?: string;
  displayQuantity?: string;
}

export interface CancelAllOrdersResponse {
  canceledOrderIds: string[];
  count: number;
}

export interface UserFill {
  tradeId: string;
  symbol: string;
  takerOrderId: string;
  makerOrderId: string;
  price: string;
  qty: string;
  takerSide: OrderSide;
  ts: string;
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

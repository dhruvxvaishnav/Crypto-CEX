import { api } from "@/lib/api-client";
import type {
  ApiDataResponse,
  Kline,
  Market,
  OrderBookDelta,
  OrderBookSnapshot,
  PriceLevelTuple,
  Trade,
} from "@/types/api.types";

const DEFAULT_ORDERBOOK_DEPTH = 100;
const DEFAULT_TRADES_LIMIT = 80;
const DEFAULT_KLINES_LIMIT = 500;
const DEFAULT_KLINE_INTERVAL = "1m";

export interface ReplayTradesParams {
  from: number;
  to: number;
  limit?: number;
}

export async function getMarkets(): Promise<Market[]> {
  const response = await api.get<ApiDataResponse<Market[]>>("/markets");
  return response.data;
}

export async function getOrderBook(symbol: string): Promise<OrderBookSnapshot> {
  return api.get<OrderBookSnapshot>(
    `/markets/${encodeURIComponent(symbol)}/orderbook?depth=${DEFAULT_ORDERBOOK_DEPTH}`,
  );
}

export async function getRecentTrades(symbol: string): Promise<Trade[]> {
  const response = await api.get<ApiDataResponse<Trade[]>>(
    `/markets/${encodeURIComponent(symbol)}/trades?limit=${DEFAULT_TRADES_LIMIT}`,
  );
  return response.data;
}

export async function getReplayTrades(
  symbol: string,
  params: ReplayTradesParams,
): Promise<Trade[]> {
  const query = new URLSearchParams({
    from: params.from.toString(),
    limit: (params.limit ?? 1000).toString(),
    order: "asc",
    to: params.to.toString(),
  });
  const response = await api.get<ApiDataResponse<Trade[]>>(
    `/markets/${encodeURIComponent(symbol)}/trades?${query.toString()}`,
  );
  return response.data;
}

export async function getKlines(symbol: string): Promise<Kline[]> {
  const response = await api.get<ApiDataResponse<Kline[]>>(
    `/markets/${encodeURIComponent(symbol)}/klines?interval=${DEFAULT_KLINE_INTERVAL}&limit=${DEFAULT_KLINES_LIMIT}`,
  );
  return response.data;
}

export function isOrderBookDelta(value: unknown): value is OrderBookDelta {
  if (!isRecord(value)) return false;
  return typeof value.seq === "number" && isPriceLevels(value.bids) && isPriceLevels(value.asks);
}

export function isTrade(value: unknown): value is Trade {
  if (!isRecord(value)) return false;
  return (
    typeof value.id === "string" &&
    typeof value.price === "string" &&
    typeof value.qty === "string" &&
    (value.side === "buy" || value.side === "sell") &&
    typeof value.ts === "string"
  );
}

function isPriceLevels(value: unknown): value is PriceLevelTuple[] {
  return (
    Array.isArray(value) &&
    value.every(
      (level) =>
        Array.isArray(level) &&
        level.length === 2 &&
        typeof level[0] === "string" &&
        typeof level[1] === "string",
    )
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

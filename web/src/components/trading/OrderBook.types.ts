import type { UseWebSocketReturn } from "@/lib/ws-client";

export interface OrderBookProps {
  symbol: string;
  ws: UseWebSocketReturn;
}

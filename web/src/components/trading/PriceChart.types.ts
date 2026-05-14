import type { UseWebSocketReturn } from "@/lib/ws-client";

export interface PriceChartProps {
  symbol: string;
  ws: UseWebSocketReturn;
}

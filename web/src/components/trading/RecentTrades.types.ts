import type { UseWebSocketReturn } from "@/lib/ws-client";

export interface RecentTradesProps {
  symbol: string;
  ws: UseWebSocketReturn;
}

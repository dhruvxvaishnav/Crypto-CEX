import type { UseWebSocketReturn } from "@/lib/ws-client";
import type { Order, UserFill } from "@/types/api.types";

export interface TradeActivityProps {
  symbol: string;
  ws: UseWebSocketReturn;
  latestOrder: Order | null;
}

export interface OrdersTableProps {
  orders: Order[];
  emptyLabel: string;
  onCancel?: (orderId: string) => void;
  pendingCancelId?: string | null;
}

export interface FillsTableProps {
  fills: UserFill[];
}

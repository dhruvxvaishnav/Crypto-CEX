import { api } from "@/lib/api-client";
import type {
  ApiListResponse,
  CancelAllOrdersResponse,
  Order,
  PlaceOrderInput,
} from "@/types/api.types";

interface ListOrdersParams {
  market: string;
  limit?: number;
}

const DEFAULT_ORDER_LIMIT = 100;

export async function placeOrder(input: PlaceOrderInput): Promise<Order> {
  return api.post<Order>("/orders", input);
}

export async function getOrders(params: ListOrdersParams): Promise<Order[]> {
  const limit = params.limit ?? DEFAULT_ORDER_LIMIT;
  const query = new URLSearchParams({
    market: params.market,
    limit: limit.toString(),
  });
  const response = await api.get<ApiListResponse<Order>>(`/orders?${query.toString()}`);
  return response.data;
}

export async function cancelOrder(orderId: string): Promise<Order> {
  return api.delete<Order>(`/orders/${encodeURIComponent(orderId)}`);
}

export async function cancelAllOrders(market: string): Promise<CancelAllOrdersResponse> {
  const query = new URLSearchParams({ market });
  return api.delete<CancelAllOrdersResponse>(`/orders?${query.toString()}`);
}

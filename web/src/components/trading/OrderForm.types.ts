import type { Market, Order, OrderSide, OrderType } from "@/types/api.types";

export interface OrderFormValues {
  side: OrderSide;
  orderType: OrderType;
  price: string;
  stopPrice: string;
  quantity: string;
  quoteQuantity: string;
  displayQuantity: string;
  iceberg: boolean;
}

export interface OrderFormProps {
  market: Market | null;
  symbol: string;
  side: OrderSide;
  formId: string;
  onSideChange: (side: OrderSide) => void;
  onPlaced: (order: Order) => void;
}

export interface OrderTypeOption {
  value: OrderType;
  label: string;
}

export type ClientOrderIdFactory = () => string;

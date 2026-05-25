import { z } from "zod";

import type { Market, PlaceOrderInput } from "@/types/api.types";

import type { ClientOrderIdFactory, OrderFormValues, OrderTypeOption } from "./OrderForm.types";
import { compareDecimalStrings } from "./order-book-model";

export const ORDER_TYPE_OPTIONS: OrderTypeOption[] = [
  { value: "limit", label: "Limit" },
  { value: "market", label: "Market" },
  { value: "ioc", label: "IOC" },
  { value: "fok", label: "FOK" },
  { value: "post_only", label: "Post-only" },
  { value: "stop_limit", label: "Stop limit" },
  { value: "stop_market", label: "Stop market" },
  { value: "oco", label: "OCO" },
];

const DECIMAL_RE = /^(?:0|[1-9]\d*)(?:\.\d+)?$/;

export const DEFAULT_ORDER_FORM_VALUES: OrderFormValues = {
  side: "buy",
  orderType: "limit",
  price: "",
  stopPrice: "",
  quantity: "",
  quoteQuantity: "",
  displayQuantity: "",
  iceberg: false,
};

export const orderFormSchema = z
  .object({
    side: z.enum(["buy", "sell"]),
    orderType: z.enum([
      "limit",
      "market",
      "ioc",
      "fok",
      "post_only",
      "stop_limit",
      "stop_market",
      "oco",
    ]),
    price: z.string(),
    stopPrice: z.string(),
    quantity: z.string(),
    quoteQuantity: z.string(),
    displayQuantity: z.string(),
    iceberg: z.boolean(),
  })
  .superRefine((values, ctx) => {
    validateDecimalField(ctx, values.price, "price", "Price");
    validateDecimalField(ctx, values.stopPrice, "stopPrice", "Stop price");
    validateDecimalField(ctx, values.quantity, "quantity", "Quantity");
    validateDecimalField(ctx, values.quoteQuantity, "quoteQuantity", "Quote quantity");
    validateDecimalField(ctx, values.displayQuantity, "displayQuantity", "Display quantity");

    if (requiresPrice(values.orderType) && values.price.trim() === "") {
      addIssue(ctx, "price", "Price is required");
    }
    if (requiresStopPrice(values.orderType) && values.stopPrice.trim() === "") {
      addIssue(ctx, "stopPrice", "Stop price is required");
    }
    if (requiresQuoteQuantity(values) && values.quoteQuantity.trim() === "") {
      addIssue(ctx, "quoteQuantity", "Quote quantity is required");
    }
    if (!requiresQuoteQuantity(values) && values.quantity.trim() === "") {
      addIssue(ctx, "quantity", "Quantity is required");
    }
    if (values.iceberg && values.orderType === "limit" && values.displayQuantity.trim() === "") {
      addIssue(ctx, "displayQuantity", "Display quantity is required");
    }
    if (
      values.iceberg &&
      values.orderType === "limit" &&
      values.quantity.trim() !== "" &&
      values.displayQuantity.trim() !== "" &&
      compareDecimalStrings(values.displayQuantity, values.quantity) >= 0
    ) {
      addIssue(ctx, "displayQuantity", "Display quantity must be smaller");
    }
  });

export function buildPlaceOrderPayload(
  values: OrderFormValues,
  market: Market,
  createClientOrderId: ClientOrderIdFactory,
): PlaceOrderInput {
  const payload = {
    clientOrderId: `web-${createClientOrderId()}`,
    market: market.symbol,
    side: values.side,
    type: values.orderType,
  };

  const withOptionalFields = {
    ...payload,
    ...(requiresPrice(values.orderType) ? { price: values.price.trim() } : {}),
    ...(requiresStopPrice(values.orderType) ? { stopPrice: values.stopPrice.trim() } : {}),
    ...(requiresQuoteQuantity(values) ? { quoteQuantity: values.quoteQuantity.trim() } : {}),
    ...(!requiresQuoteQuantity(values) ? { quantity: values.quantity.trim() } : {}),
    ...(values.iceberg && values.orderType === "limit"
      ? { displayQuantity: values.displayQuantity.trim() }
      : {}),
  };

  return withOptionalFields;
}

export function requiresPrice(orderType: OrderFormValues["orderType"]): boolean {
  return ["limit", "ioc", "fok", "post_only", "stop_limit", "oco"].includes(orderType);
}

export function requiresStopPrice(orderType: OrderFormValues["orderType"]): boolean {
  return ["stop_limit", "stop_market", "oco"].includes(orderType);
}

export function requiresQuoteQuantity(
  values: Pick<OrderFormValues, "side" | "orderType">,
): boolean {
  return values.side === "buy" && values.orderType === "market";
}

export function createBrowserClientOrderId(): string {
  return crypto.randomUUID();
}

function validateDecimalField(
  ctx: z.RefinementCtx,
  value: string,
  path: keyof OrderFormValues,
  label: string,
) {
  const trimmed = value.trim();
  if (trimmed === "") return;
  if (!DECIMAL_RE.test(trimmed) || compareDecimalStrings(trimmed, "0") <= 0) {
    addIssue(ctx, path, `${label} must be a positive decimal`);
  }
}

function addIssue(ctx: z.RefinementCtx, path: keyof OrderFormValues, message: string) {
  ctx.addIssue({
    code: "custom",
    path: [path],
    message,
  });
}

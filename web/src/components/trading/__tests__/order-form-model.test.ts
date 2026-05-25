import { describe, expect, it } from "vitest";

import type { Market } from "@/types/api.types";

import type { OrderFormValues } from "../OrderForm.types";
import { buildPlaceOrderPayload, orderFormSchema } from "../order-form-model";

const MARKET: Market = {
  id: "market-1",
  symbol: "BTCUSDT",
  baseAsset: "BTC",
  quoteAsset: "USDT",
  status: "trading",
  tickSize: "0.01",
  lotSize: "0.0001",
  minNotional: "10",
  lastPrice: "100",
  volume24h: null,
  high24h: null,
  low24h: null,
  priceChangePct: null,
  makerFeeBps: "10",
  takerFeeBps: "10",
};

describe("order-form-model", () => {
  it("builds a market buy payload with quote quantity", () => {
    const values = baseValues({ orderType: "market", side: "buy", quoteQuantity: "25" });

    const result = buildPlaceOrderPayload(values, MARKET, () => "id-1");

    expect(result).toEqual({
      clientOrderId: "web-id-1",
      market: "BTCUSDT",
      side: "buy",
      type: "market",
      quoteQuantity: "25",
    });
  });

  it("requires stop and limit prices for oco orders", () => {
    const result = orderFormSchema.safeParse(baseValues({ orderType: "oco", quantity: "1" }));

    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues.map((issue) => issue.path[0])).toEqual(["price", "stopPrice"]);
    }
  });

  it("rejects an iceberg display quantity that is not smaller than total quantity", () => {
    const result = orderFormSchema.safeParse(
      baseValues({
        orderType: "limit",
        price: "100",
        quantity: "1",
        displayQuantity: "1",
        iceberg: true,
      }),
    );

    expect(result.success).toBe(false);
  });
});

function baseValues(overrides: Partial<OrderFormValues>): OrderFormValues {
  return {
    side: "sell",
    orderType: "limit",
    price: "",
    stopPrice: "",
    quantity: "",
    quoteQuantity: "",
    displayQuantity: "",
    iceberg: false,
    ...overrides,
  };
}

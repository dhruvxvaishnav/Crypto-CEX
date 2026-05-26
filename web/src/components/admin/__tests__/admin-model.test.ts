import { describe, expect, it } from "vitest";

import type { AdminEngineMarketState } from "@/types/api.types";

import { statusTone, topOfBookText, totalOpenOrders } from "../admin-model";

describe("admin-model", () => {
  it("maps market status to operational tones", () => {
    expect(statusTone("trading")).toBe("success");
    expect(statusTone("halted")).toBe("danger");
    expect(statusTone("delisted")).toBe("muted");
  });

  it("summarises open orders across markets", () => {
    expect(
      totalOpenOrders([
        market({ openOrderCount: 2, symbol: "BTCUSDT" }),
        market({ openOrderCount: 3, symbol: "ETHUSDT" }),
      ]),
    ).toBe(5);
  });

  it("formats top-of-book price levels", () => {
    expect(topOfBookText(["100", "0.25"])).toBe("100 x 0.25");
    expect(topOfBookText(null)).toBe("--");
  });
});

function market(overrides: Partial<AdminEngineMarketState>): AdminEngineMarketState {
  return {
    bestAsk: null,
    bestBid: null,
    openOrderCount: 0,
    seq: null,
    status: "trading",
    symbol: "BTCUSDT",
    ...overrides,
  };
}

import { describe, expect, it } from "vitest";

import type { Balance, Market } from "@/types/api.types";

import {
  estimatePortfolioValue,
  formatDecimal,
  validateFaucetInput,
  validateWithdrawInput,
} from "../account-model";

const MARKETS: Market[] = [
  market({ symbol: "BTCUSDT", baseAsset: "BTC", lastPrice: "100000" }),
  market({ symbol: "ETHUSDT", baseAsset: "ETH", lastPrice: "3000" }),
];

describe("account-model", () => {
  it("estimates portfolio value from USDT quote markets without numeric money coercion", () => {
    const balances: Balance[] = [
      balance({ asset: "BTC", total: "0.5" }),
      balance({ asset: "ETH", total: "2" }),
      balance({ asset: "USDT", total: "125.25" }),
    ];

    const result = estimatePortfolioValue(balances, MARKETS);

    expect(result.totalUsd).toBe("56125.25");
    expect(result.pricedAssetCount).toBe(3);
  });

  it("validates faucet amount as a positive decimal", () => {
    expect(validateFaucetInput("USDT", "100")).toBeNull();
    expect(validateFaucetInput("USDT", "0")).toBe("Amount must be a positive decimal");
  });

  it("validates withdrawal address and minimum by asset", () => {
    const invalid = validateWithdrawInput({ asset: "ETH", amount: "0.0001", address: "nope" });
    const valid = validateWithdrawInput({
      asset: "ETH",
      amount: "0.01",
      address: "0x0000000000000000000000000000000000000000",
    });

    expect(invalid.canSubmit).toBe(false);
    expect(invalid.amountError).toBe("Minimum withdrawal is 0.001 ETH");
    expect(invalid.addressError).toBe("Enter a valid ERC-20 address");
    expect(valid.canSubmit).toBe(true);
  });

  it("formats long decimal strings for compact account display", () => {
    expect(formatDecimal("1.230000000000000000")).toBe("1.23");
    expect(formatDecimal("100.000000000000000000")).toBe("100");
  });
});

function balance(overrides: Partial<Balance>): Balance {
  return {
    asset: "USDT",
    assetName: "Tether USD",
    available: overrides.total ?? "0",
    locked: "0",
    total: "0",
    ...overrides,
  };
}

function market(overrides: Partial<Market>): Market {
  return {
    id: overrides.symbol ?? "BTCUSDT",
    symbol: "BTCUSDT",
    baseAsset: "BTC",
    quoteAsset: "USDT",
    status: "trading",
    tickSize: "0.01",
    lotSize: "0.0001",
    minNotional: "10",
    lastPrice: "1",
    volume24h: null,
    high24h: null,
    low24h: null,
    priceChangePct: null,
    makerFeeBps: "10",
    takerFeeBps: "10",
    ...overrides,
  };
}

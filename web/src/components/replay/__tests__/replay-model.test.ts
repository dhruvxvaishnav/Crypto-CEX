import { describe, expect, it } from "vitest";

import type { Trade } from "@/types/api.types";

import { buildReplayFrames, normalizeReplaySpeed, parseReplayRange } from "../replay-model";

describe("replay-model", () => {
  it("normalises replay query params with a one-minute window", () => {
    expect(parseReplayRange("1700000000", "4", 1800000000)).toEqual({
      from: 1700000000,
      speed: 4,
      to: 1700000060,
    });
    expect(parseReplayRange(null, "99", 1800000000)).toEqual({
      from: 1799999940,
      speed: 1,
      to: 1800000000,
    });
    expect(normalizeReplaySpeed("16")).toBe(16);
  });

  it("builds deterministic synthetic book frames from chronological trades", () => {
    const frames = buildReplayFrames([
      trade({ id: "1", price: "100.10", qty: "0.2", side: "buy" }),
      trade({ id: "2", price: "100.20", qty: "0.3", side: "sell" }),
      trade({ id: "3", price: "100.10", qty: "0.4", side: "buy" }),
    ]);

    expect(frames).toHaveLength(3);
    expect(frames[2]?.bids.map((row) => [row.price, row.quantity, row.total])).toEqual([
      ["100.10", "0.6", "0.6"],
    ]);
    expect(frames[2]?.asks.map((row) => [row.price, row.quantity, row.total])).toEqual([
      ["100.20", "0.3", "0.3"],
    ]);
  });

  it("sorts bid levels descending and ask levels ascending", () => {
    const frames = buildReplayFrames([
      trade({ id: "1", price: "100.10", side: "buy" }),
      trade({ id: "2", price: "100.20", side: "buy" }),
      trade({ id: "3", price: "100.50", side: "sell" }),
      trade({ id: "4", price: "100.40", side: "sell" }),
    ]);

    expect(frames.at(-1)?.bids.map((row) => row.price)).toEqual(["100.20", "100.10"]);
    expect(frames.at(-1)?.asks.map((row) => row.price)).toEqual(["100.40", "100.50"]);
  });
});

function trade(overrides: Partial<Trade>): Trade {
  return {
    id: "1",
    price: "100",
    qty: "0.1",
    side: "buy",
    ts: "2026-05-26T00:00:00Z",
    ...overrides,
  };
}

import { describe, expect, it } from "vitest";

import type { OrderBookDelta, OrderBookSnapshot } from "@/types/api.types";

import { compareDecimalStrings, mergeBookDelta, snapshotToState } from "../order-book-model";

describe("order-book-model", () => {
  it("sorts bid and ask sides without numeric money coercion", () => {
    const snapshot: OrderBookSnapshot = {
      seq: 1,
      bids: [
        ["100.01", "1"],
        ["100.10", "2"],
      ],
      asks: [
        ["101.05", "1"],
        ["101.01", "2"],
      ],
    };

    const state = snapshotToState(snapshot);

    expect(state.bids.map((row) => row.price)).toEqual(["100.10", "100.01"]);
    expect(state.asks.map((row) => row.price)).toEqual(["101.01", "101.05"]);
  });

  it("applies delta updates, inserts, and zero-quantity deletes", () => {
    const snapshot: OrderBookSnapshot = {
      seq: 10,
      bids: [
        ["100", "1"],
        ["99", "2"],
      ],
      asks: [["101", "1"]],
    };
    const delta: OrderBookDelta = {
      seq: 11,
      bids: [
        ["100", "0"],
        ["98", "4"],
      ],
      asks: [["101", "3"]],
    };

    const state = mergeBookDelta(snapshotToState(snapshot), delta);

    expect(state.seq).toBe(11);
    expect(state.bids.map((row) => [row.price, row.quantity])).toEqual([
      ["99", "2"],
      ["98", "4"],
    ]);
    expect(state.asks.map((row) => [row.price, row.quantity])).toEqual([["101", "3"]]);
  });

  it("ignores stale deltas", () => {
    const state = snapshotToState({ seq: 4, bids: [["100", "1"]], asks: [] });
    const stale = mergeBookDelta(state, { seq: 4, bids: [["100", "0"]], asks: [] });

    expect(stale).toBe(state);
  });

  it("compares decimal strings by value", () => {
    expect(compareDecimalStrings("100.02", "100.010")).toBe(1);
    expect(compareDecimalStrings("099.50", "100")).toBe(-1);
    expect(compareDecimalStrings("1.2300", "1.23")).toBe(0);
  });
});

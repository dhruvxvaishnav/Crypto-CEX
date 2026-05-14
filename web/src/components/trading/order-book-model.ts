import type { OrderBookDelta, OrderBookSnapshot, PriceLevelTuple } from "@/types/api.types";

export interface BookSideRow {
  price: string;
  quantity: string;
  total: string;
  depthPct: number;
}

export interface OrderBookState {
  seq: number;
  bids: BookSideRow[];
  asks: BookSideRow[];
}

type Side = "bids" | "asks";
type MutableLevel = [string, string];

const ZERO_DECIMAL_RE = /^0+(?:\.0+)?$/;

export function snapshotToState(snapshot: OrderBookSnapshot): OrderBookState {
  return {
    seq: snapshot.seq,
    bids: levelsToRows(sortLevels(snapshot.bids, "bids")),
    asks: levelsToRows(sortLevels(snapshot.asks, "asks")),
  };
}

export function mergeBookDelta(state: OrderBookState, delta: OrderBookDelta): OrderBookState {
  if (delta.seq <= state.seq) return state;

  const bids = applySideDelta(state.bids, delta.bids, "bids");
  const asks = applySideDelta(state.asks, delta.asks, "asks");

  return {
    seq: delta.seq,
    bids: levelsToRows(bids),
    asks: levelsToRows(asks),
  };
}

export function compareDecimalStrings(left: string, right: string): number {
  const leftParts = normalizeDecimal(left);
  const rightParts = normalizeDecimal(right);

  if (leftParts.integer.length !== rightParts.integer.length) {
    return leftParts.integer.length > rightParts.integer.length ? 1 : -1;
  }

  if (leftParts.integer !== rightParts.integer) {
    return leftParts.integer > rightParts.integer ? 1 : -1;
  }

  const maxFractionLength = Math.max(leftParts.fraction.length, rightParts.fraction.length);
  const leftFraction = leftParts.fraction.padEnd(maxFractionLength, "0");
  const rightFraction = rightParts.fraction.padEnd(maxFractionLength, "0");

  if (leftFraction === rightFraction) return 0;
  return leftFraction > rightFraction ? 1 : -1;
}

function applySideDelta(
  rows: BookSideRow[],
  changes: readonly PriceLevelTuple[],
  side: Side,
): MutableLevel[] {
  const byPrice = new Map<string, string>(rows.map((row) => [row.price, row.quantity]));
  for (const [price, quantity] of changes) {
    if (isZeroDecimal(quantity)) {
      byPrice.delete(price);
    } else {
      byPrice.set(price, quantity);
    }
  }
  return sortLevels(Array.from(byPrice.entries()), side);
}

function sortLevels(levels: readonly PriceLevelTuple[], side: Side): MutableLevel[] {
  return levels
    .map(([price, quantity]) => [price, quantity] satisfies MutableLevel)
    .sort(([left], [right]) =>
      side === "bids" ? compareDecimalStrings(right, left) : compareDecimalStrings(left, right),
    );
}

function levelsToRows(levels: readonly PriceLevelTuple[]): BookSideRow[] {
  let total = "0";
  const totals = levels.map(([_, quantity]) => {
    total = addDecimalStrings(total, quantity);
    return total;
  });
  const maxTotal = totals.at(-1) ?? "0";

  return levels.map(([price, quantity], index) => ({
    price,
    quantity,
    total: totals[index] ?? quantity,
    depthPct: depthPct(totals[index] ?? quantity, maxTotal),
  }));
}

function addDecimalStrings(left: string, right: string): string {
  const leftParts = normalizeDecimal(left);
  const rightParts = normalizeDecimal(right);
  const scale = Math.max(leftParts.fraction.length, rightParts.fraction.length);
  const leftScaled = BigInt(`${leftParts.integer}${leftParts.fraction.padEnd(scale, "0")}`);
  const rightScaled = BigInt(`${rightParts.integer}${rightParts.fraction.padEnd(scale, "0")}`);
  const sum = (leftScaled + rightScaled).toString().padStart(scale + 1, "0");

  if (scale === 0) return sum;
  const integer = sum.slice(0, -scale) || "0";
  const fraction = sum.slice(-scale).replace(/0+$/, "");
  return fraction ? `${integer}.${fraction}` : integer;
}

function normalizeDecimal(value: string): { integer: string; fraction: string } {
  const [rawInteger = "0", rawFraction = ""] = value.split(".");
  const integer = rawInteger.replace(/^0+/, "") || "0";
  return { integer, fraction: rawFraction.replace(/0+$/, "") };
}

function isZeroDecimal(value: string): boolean {
  return ZERO_DECIMAL_RE.test(value);
}

function depthPct(total: string, maxTotal: string): number {
  if (isZeroDecimal(maxTotal)) return 0;
  return Math.min((Number(total) / Number(maxTotal)) * 100, 100);
}

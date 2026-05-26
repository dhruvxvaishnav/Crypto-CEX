import type { BookSideRow } from "@/components/trading/order-book-model";
import { compareDecimalStrings } from "@/components/trading/order-book-model";
import type { Trade } from "@/types/api.types";

export type ReplaySpeed = 1 | 4 | 16;

export interface ReplayFrame {
  index: number;
  trade: Trade;
  bids: BookSideRow[];
  asks: BookSideRow[];
}

export interface ReplayRange {
  from: number;
  speed: ReplaySpeed;
  to: number;
}

type BookSide = "bids" | "asks";
type MutableLevel = [string, string];

const DEFAULT_WINDOW_SECONDS = 60;
const DEFAULT_SPEED: ReplaySpeed = 1;
const SPEEDS: ReplaySpeed[] = [1, 4, 16];
const ZERO_DECIMAL_RE = /^0+(?:\.0+)?$/;

export function parseReplayRange(
  fromParam: string | null,
  speedParam: string | null,
  fallbackNowSeconds: number,
): ReplayRange {
  const parsedFrom = fromParam ? Number.parseInt(fromParam, 10) : Number.NaN;
  const from = Number.isFinite(parsedFrom)
    ? parsedFrom
    : fallbackNowSeconds - DEFAULT_WINDOW_SECONDS;
  return {
    from,
    speed: normalizeReplaySpeed(speedParam),
    to: from + DEFAULT_WINDOW_SECONDS,
  };
}

export function normalizeReplaySpeed(value: string | null): ReplaySpeed {
  const parsed = value ? Number.parseInt(value, 10) : DEFAULT_SPEED;
  return SPEEDS.find((speed) => speed === parsed) ?? DEFAULT_SPEED;
}

export function buildReplayFrames(trades: Trade[], depth = 10): ReplayFrame[] {
  const bids = new Map<string, string>();
  const asks = new Map<string, string>();

  return trades.map((trade, index) => {
    const side = trade.side === "buy" ? bids : asks;
    const current = side.get(trade.price) ?? "0";
    side.set(trade.price, addDecimalStrings(current, trade.qty));

    return {
      index,
      trade,
      bids: levelsToRows(sortLevels(Array.from(bids.entries()), "bids").slice(0, depth)),
      asks: levelsToRows(sortLevels(Array.from(asks.entries()), "asks").slice(0, depth)),
    };
  });
}

function sortLevels(levels: readonly MutableLevel[], side: BookSide): MutableLevel[] {
  return levels
    .filter(([, quantity]) => !isZeroDecimal(quantity))
    .sort(([left], [right]) =>
      side === "bids" ? compareDecimalStrings(right, left) : compareDecimalStrings(left, right),
    );
}

function levelsToRows(levels: readonly MutableLevel[]): BookSideRow[] {
  let total = "0";
  const totals = levels.map(([, quantity]) => {
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

function normalizeDecimal(value: string): { fraction: string; integer: string } {
  const [rawInteger = "0", rawFraction = ""] = value.split(".");
  const integer = rawInteger.replace(/^0+/, "") || "0";
  return { fraction: rawFraction.replace(/0+$/, ""), integer };
}

function isZeroDecimal(value: string): boolean {
  return ZERO_DECIMAL_RE.test(value);
}

function depthPct(total: string, maxTotal: string): number {
  if (isZeroDecimal(maxTotal)) return 0;
  return Math.min((Number(total) / Number(maxTotal)) * 100, 100);
}

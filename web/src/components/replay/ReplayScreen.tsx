"use client";

import { useQuery } from "@tanstack/react-query";
import type { CandlestickData, IChartApi, ISeriesApi, UTCTimestamp } from "lightweight-charts";
import { Gauge, Pause, Play, RotateCcw, SkipForward } from "lucide-react";
import { useSearchParams } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";

import type { MarketSelectorOption } from "@/components/layout/MarketSelector.types";
import { TopBar } from "@/components/layout/TopBar";
import type { MarketStats } from "@/components/layout/TopBar.types";
import type { BookSideRow } from "@/components/trading/order-book-model";
import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { Button } from "@/components/ui/Button";
import { getMarkets, getReplayTrades } from "@/lib/market-data";
import type { Market, Trade } from "@/types/api.types";
import type { ReplayScreenProps } from "./ReplayScreen.types";
import type { ReplayFrame, ReplaySpeed } from "./replay-model";
import { buildReplayFrames, parseReplayRange } from "./replay-model";

const PLAYBACK_BASE_MS = 1000;
const SPEEDS: ReplaySpeed[] = [1, 4, 16];

export function ReplayScreen({ initialSymbol }: ReplayScreenProps) {
  const symbol = initialSymbol.toUpperCase();
  const searchParams = useSearchParams();
  const [fallbackNowSeconds] = useState(() => Math.floor(Date.now() / 1000));
  const range = useMemo(
    () => parseReplayRange(searchParams.get("from"), searchParams.get("speed"), fallbackNowSeconds),
    [fallbackNowSeconds, searchParams],
  );
  const [speed, setSpeed] = useState<ReplaySpeed>(range.speed);
  const [activeIndex, setActiveIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);

  const marketsQuery = useQuery({ queryKey: ["markets"], queryFn: getMarkets });
  const tradesQuery = useQuery({
    queryKey: ["replay-trades", symbol, range.from, range.to],
    queryFn: () => getReplayTrades(symbol, { from: range.from, to: range.to }),
  });

  const markets = marketsQuery.data ?? [];
  const activeMarket = markets.find((market) => market.symbol === symbol) ?? null;
  const trades = tradesQuery.data ?? [];
  const frames = useMemo(() => buildReplayFrames(trades), [trades]);
  const activeFrame = frames[activeIndex] ?? null;
  const visibleTrades = useMemo(() => trades.slice(0, activeIndex + 1), [activeIndex, trades]);
  const replayKey = `${symbol}:${range.from}:${range.to}`;

  useEffect(() => {
    if (replayKey === "") return;
    setSpeed(range.speed);
    setActiveIndex(0);
    setIsPlaying(false);
  }, [range.speed, replayKey]);

  useEffect(() => {
    if (!isPlaying || frames.length === 0) return;
    if (activeIndex >= frames.length - 1) {
      setIsPlaying(false);
      return;
    }

    const interval = window.setInterval(() => {
      setActiveIndex((current) => Math.min(current + 1, frames.length - 1));
    }, PLAYBACK_BASE_MS / speed);
    return () => window.clearInterval(interval);
  }, [activeIndex, frames.length, isPlaying, speed]);

  function resetReplay() {
    setIsPlaying(false);
    setActiveIndex(0);
  }

  function stepReplay() {
    setIsPlaying(false);
    setActiveIndex((current) => Math.min(current + 1, Math.max(frames.length - 1, 0)));
  }

  return (
    <div className="flex h-screen min-h-0 flex-col bg-zinc-950 text-zinc-100">
      <TopBar
        activeSymbol={symbol}
        marketStats={activeMarket ? toMarketStats(activeMarket) : null}
        markets={markets.map(toSelectorOption)}
      />
      <main className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[auto_minmax(360px,1fr)_minmax(320px,0.9fr)] overflow-hidden xl:grid-cols-[320px_minmax(0,1fr)_360px] xl:grid-rows-[auto_minmax(0,1fr)]">
        <ReplayControls
          activeIndex={activeIndex}
          frameCount={frames.length}
          from={range.from}
          isPlaying={isPlaying}
          onIndexChange={setActiveIndex}
          onPlayToggle={() => setIsPlaying((current) => !current)}
          onReset={resetReplay}
          onSpeedChange={setSpeed}
          onStep={stepReplay}
          speed={speed}
          to={range.to}
        />
        <ReplayBook frame={activeFrame} />
        <ReplayChart trades={visibleTrades} symbol={symbol} />
        <ReplayTape
          activeTradeId={activeFrame?.trade.id ?? null}
          isLoading={tradesQuery.isLoading}
          trades={trades}
        />
      </main>
    </div>
  );
}

function ReplayControls({
  activeIndex,
  frameCount,
  from,
  isPlaying,
  onIndexChange,
  onPlayToggle,
  onReset,
  onSpeedChange,
  onStep,
  speed,
  to,
}: {
  activeIndex: number;
  frameCount: number;
  from: number;
  isPlaying: boolean;
  onIndexChange: (index: number) => void;
  onPlayToggle: () => void;
  onReset: () => void;
  onSpeedChange: (speed: ReplaySpeed) => void;
  onStep: () => void;
  speed: ReplaySpeed;
  to: number;
}) {
  const maxIndex = Math.max(frameCount - 1, 0);
  return (
    <section className="border-b border-zinc-800 bg-zinc-950 px-3 py-3 xl:col-span-3">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center">
        <div className="min-w-[220px]">
          <h1 className="text-sm font-semibold text-zinc-100">Replay mode</h1>
          <p className="text-xs text-zinc-500">
            {formatTime(from)} - {formatTime(to)}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button type="button" size="sm" onClick={onPlayToggle} disabled={frameCount === 0}>
            {isPlaying ? (
              <Pause className="h-3.5 w-3.5" aria-hidden />
            ) : (
              <Play className="h-3.5 w-3.5" aria-hidden />
            )}
            {isPlaying ? "Pause" : "Play"}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={onStep}
            disabled={frameCount === 0}
          >
            <SkipForward className="h-3.5 w-3.5" aria-hidden />
            Step
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={onReset}
            disabled={frameCount === 0}
          >
            <RotateCcw className="h-3.5 w-3.5" aria-hidden />
            Reset
          </Button>
        </div>
        <div className="flex items-center gap-2">
          <Gauge className="h-4 w-4 text-zinc-500" aria-hidden />
          {SPEEDS.map((option) => (
            <Button
              key={option}
              type="button"
              size="sm"
              variant={speed === option ? "primary" : "secondary"}
              onClick={() => onSpeedChange(option)}
            >
              {option}x
            </Button>
          ))}
        </div>
        <label className="flex min-w-[240px] flex-1 items-center gap-3 text-xs text-zinc-500">
          <span>
            {frameCount === 0 ? 0 : activeIndex + 1}/{frameCount}
          </span>
          <input
            type="range"
            min={0}
            max={maxIndex}
            value={activeIndex}
            disabled={frameCount === 0}
            onChange={(event) => onIndexChange(Number.parseInt(event.currentTarget.value, 10))}
            className="h-2 flex-1 accent-emerald-500"
            aria-label="Replay position"
          />
        </label>
      </div>
    </section>
  );
}

function ReplayBook({ frame }: { frame: ReplayFrame | null }) {
  return (
    <section className="min-h-0 border-r border-zinc-800 bg-zinc-950">
      <PanelHeader
        title="Sandbox book"
        subtitle={frame ? `Frame ${frame.index + 1}` : "No trades"}
      />
      <div className="grid grid-cols-[1fr_1fr_1fr] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
        <span>Price</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Total</span>
      </div>
      <BookRows rows={frame?.asks ?? []} side="asks" />
      <div className="border-y border-zinc-800 bg-zinc-900/60 px-3 py-2 text-center text-xs font-medium text-zinc-300">
        {frame?.asks[0]?.price ?? "--"} / {frame?.bids[0]?.price ?? "--"}
      </div>
      <BookRows rows={frame?.bids ?? []} side="bids" />
    </section>
  );
}

function BookRows({ rows, side }: { rows: BookSideRow[]; side: "asks" | "bids" }) {
  const sideClass = side === "bids" ? "text-emerald-400" : "text-rose-400";
  const fillClass = side === "bids" ? "bg-emerald-500/10" : "bg-rose-500/10";
  return (
    <div className="h-72 overflow-hidden">
      {rows.map((row) => (
        <div
          key={`${side}-${row.price}`}
          className="relative grid grid-cols-[1fr_1fr_1fr] px-3 text-[11px] leading-6"
        >
          <span
            className={`absolute inset-y-0 right-0 ${fillClass}`}
            style={{ width: `${row.depthPct}%` }}
            aria-hidden
          />
          <span className={`relative font-medium ${sideClass}`}>{row.price}</span>
          <span className="relative text-right text-zinc-300">{row.quantity}</span>
          <span className="relative text-right text-zinc-500">{row.total}</span>
        </div>
      ))}
    </div>
  );
}

function ReplayChart({ trades, symbol }: { trades: Trade[]; symbol: string }) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const candles = useMemo(() => tradesToCandles(trades), [trades]);

  useEffect(() => {
    let disposed = false;

    async function mountChart() {
      if (!containerRef.current || chartRef.current) return;
      const { ColorType, CrosshairMode, createChart } = await import("lightweight-charts");
      if (!containerRef.current || disposed) return;
      const chart = createChart(containerRef.current, {
        autoSize: true,
        layout: {
          background: { type: ColorType.Solid, color: "#09090b" },
          textColor: "#a1a1aa",
        },
        grid: {
          horzLines: { color: "#18181b" },
          vertLines: { color: "#18181b" },
        },
        rightPriceScale: { borderColor: "#27272a" },
        timeScale: { borderColor: "#27272a", secondsVisible: true, timeVisible: true },
        crosshair: { mode: CrosshairMode.Normal },
      });
      const series = chart.addCandlestickSeries({
        borderDownColor: "#f43f5e",
        borderUpColor: "#10b981",
        downColor: "#f43f5e",
        upColor: "#10b981",
        wickDownColor: "#fb7185",
        wickUpColor: "#34d399",
      });
      chartRef.current = chart;
      seriesRef.current = series;
    }

    void mountChart();
    return () => {
      disposed = true;
      chartRef.current?.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, []);

  useEffect(() => {
    seriesRef.current?.setData(candles);
    chartRef.current?.timeScale().fitContent();
  }, [candles]);

  return (
    <section className="flex min-h-0 flex-col bg-zinc-950">
      <PanelHeader title={symbol} subtitle="Replay chart" />
      {candles.length === 0 ? (
        <div className="flex min-h-[320px] flex-1 items-center justify-center text-sm text-zinc-500">
          No replay trades in this window.
        </div>
      ) : (
        <div
          ref={containerRef}
          className="min-h-[320px] flex-1"
          role="img"
          aria-label={`${symbol} replay chart`}
        />
      )}
    </section>
  );
}

function ReplayTape({
  activeTradeId,
  isLoading,
  trades,
}: {
  activeTradeId: string | null;
  isLoading: boolean;
  trades: Trade[];
}) {
  return (
    <section className="min-h-0 border-l border-zinc-800 bg-zinc-950">
      <PanelHeader title="Trade tape" subtitle={`${trades.length} loaded`} />
      <AsyncBoundary isLoading={isLoading} isEmpty={!isLoading && trades.length === 0}>
        <div className="overflow-auto">
          <div className="grid min-w-[420px] grid-cols-[90px_1fr_1fr_90px] border-b border-zinc-900 px-3 py-2 text-[11px] text-zinc-500">
            <span>Time</span>
            <span className="text-right">Price</span>
            <span className="text-right">Qty</span>
            <span className="text-right">Side</span>
          </div>
          {trades.map((trade) => {
            const active = trade.id === activeTradeId;
            const sideClass = trade.side === "buy" ? "text-emerald-400" : "text-rose-400";
            return (
              <div
                key={trade.id}
                className={[
                  "grid min-w-[420px] grid-cols-[90px_1fr_1fr_90px] px-3 text-xs leading-8",
                  active ? "bg-emerald-500/10" : "hover:bg-zinc-900/70",
                ].join(" ")}
              >
                <span className="text-zinc-500">{formatTradeTime(trade.ts)}</span>
                <span className="text-right text-zinc-100">{trade.price}</span>
                <span className="text-right text-zinc-300">{trade.qty}</span>
                <span className={`text-right font-medium ${sideClass}`}>{trade.side}</span>
              </div>
            );
          })}
        </div>
      </AsyncBoundary>
    </section>
  );
}

function PanelHeader({ title, subtitle }: { subtitle: string; title: string }) {
  return (
    <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
      <h2 className="text-sm font-semibold text-zinc-100">{title}</h2>
      <span className="text-[11px] text-zinc-500">{subtitle}</span>
    </div>
  );
}

function tradesToCandles(trades: Trade[]): CandlestickData[] {
  const candles = new Map<number, CandlestickData>();
  for (const trade of trades) {
    const price = Number(trade.price);
    const timestamp = Math.floor(new Date(trade.ts).valueOf() / 1000);
    if (!Number.isFinite(price) || !Number.isFinite(timestamp)) continue;
    const existing = candles.get(timestamp);
    if (!existing) {
      candles.set(timestamp, {
        close: price,
        high: price,
        low: price,
        open: price,
        time: timestamp as UTCTimestamp,
      });
      continue;
    }
    candles.set(timestamp, {
      ...existing,
      close: price,
      high: Math.max(existing.high, price),
      low: Math.min(existing.low, price),
    });
  }
  return Array.from(candles.values()).sort((left, right) => Number(left.time) - Number(right.time));
}

function toSelectorOption(market: Market): MarketSelectorOption {
  return {
    baseAsset: market.baseAsset,
    lastPrice: market.lastPrice,
    priceChangePct24h: market.priceChangePct,
    quoteAsset: market.quoteAsset,
    symbol: market.symbol,
  };
}

function toMarketStats(market: Market): MarketStats {
  return {
    lastPrice: market.lastPrice,
    priceChange24h: null,
    priceChangePct24h: market.priceChangePct,
    volume24h: market.volume24h,
  };
}

function formatTime(value: number): string {
  return new Intl.DateTimeFormat("en", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    timeZone: "UTC",
  }).format(new Date(value * 1000));
}

function formatTradeTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--";
  return new Intl.DateTimeFormat("en", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    timeZone: "UTC",
  }).format(date);
}

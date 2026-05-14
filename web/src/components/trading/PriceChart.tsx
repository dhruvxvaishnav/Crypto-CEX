"use client";

import { useQuery } from "@tanstack/react-query";
import type { CandlestickData, IChartApi, ISeriesApi, UTCTimestamp } from "lightweight-charts";
import { useEffect, useMemo, useRef, useState } from "react";

import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { getKlines, isTrade } from "@/lib/market-data";

import type { PriceChartProps } from "./PriceChart.types";

const CANDLE_SECONDS = 60;

export function PriceChart({ symbol, ws }: PriceChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const [candles, setCandles] = useState<CandlestickData[]>([]);
  const klinesQuery = useQuery({
    queryKey: ["klines", symbol],
    queryFn: () => getKlines(symbol),
  });
  const shouldMountChart = !klinesQuery.isLoading && candles.length > 0;

  const historicalCandles = useMemo(
    () =>
      (klinesQuery.data ?? []).map((kline) => ({
        time: kline.ts as UTCTimestamp,
        open: Number(kline.open),
        high: Number(kline.high),
        low: Number(kline.low),
        close: Number(kline.close),
      })),
    [klinesQuery.data],
  );

  useEffect(() => {
    setCandles(historicalCandles);
  }, [historicalCandles]);

  useEffect(() => {
    return ws.subscribe(`trade.${symbol}`, (payload) => {
      if (!isTrade(payload)) return;
      setCandles((current) => mergeLiveTrade(current, payload.price, payload.ts));
    });
  }, [symbol, ws.subscribe]);

  useEffect(() => {
    if (!shouldMountChart) return;

    let resizeObserver: ResizeObserver | null = null;
    let disposed = false;

    async function mountChart() {
      if (!containerRef.current) return;
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
        rightPriceScale: {
          borderColor: "#27272a",
        },
        timeScale: {
          borderColor: "#27272a",
          timeVisible: true,
          secondsVisible: false,
        },
        crosshair: {
          mode: CrosshairMode.Normal,
        },
      });
      const series = chart.addCandlestickSeries({
        upColor: "#10b981",
        downColor: "#f43f5e",
        borderUpColor: "#10b981",
        borderDownColor: "#f43f5e",
        wickUpColor: "#34d399",
        wickDownColor: "#fb7185",
      });

      chartRef.current = chart;
      seriesRef.current = series;
      chart.timeScale().fitContent();

      resizeObserver = new ResizeObserver(() => chart.timeScale().fitContent());
      resizeObserver.observe(containerRef.current);
    }

    void mountChart();

    return () => {
      disposed = true;
      resizeObserver?.disconnect();
      chartRef.current?.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, [shouldMountChart]);

  useEffect(() => {
    seriesRef.current?.setData(candles);
  }, [candles]);

  return (
    <section className="flex min-h-[420px] flex-col bg-zinc-950">
      <div className="flex h-10 items-center justify-between border-b border-zinc-800 px-3">
        <div>
          <h2 className="text-sm font-semibold text-zinc-100">{symbol}</h2>
          <p className="text-[11px] text-zinc-500">1m candles</p>
        </div>
      </div>
      <AsyncBoundary
        isLoading={klinesQuery.isLoading && candles.length === 0}
        isEmpty={!klinesQuery.isLoading && candles.length === 0}
      >
        <div
          ref={containerRef}
          className="min-h-[380px] flex-1"
          role="img"
          aria-label={`${symbol} chart`}
        />
      </AsyncBoundary>
    </section>
  );
}

function mergeLiveTrade(
  candles: CandlestickData[],
  priceText: string,
  ts: string,
): CandlestickData[] {
  const price = Number(priceText);
  const tradeMs = new Date(ts).valueOf();
  if (!Number.isFinite(price) || Number.isNaN(tradeMs)) return candles;

  const candleTime = (Math.floor(tradeMs / 1000 / CANDLE_SECONDS) * CANDLE_SECONDS) as UTCTimestamp;
  const last = candles.at(-1);
  if (!last || last.time !== candleTime) {
    return [...candles, { time: candleTime, open: price, high: price, low: price, close: price }];
  }

  return [
    ...candles.slice(0, -1),
    {
      ...last,
      high: Math.max(last.high, price),
      low: Math.min(last.low, price),
      close: price,
    },
  ];
}

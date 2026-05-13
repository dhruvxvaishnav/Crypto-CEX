import { ArrowRight, BarChart2, Shield, Zap } from "lucide-react";
import Link from "next/link";

const FEATURES = [
  {
    icon: Zap,
    title: "Sub-millisecond matching",
    body: "Rust matching engine with write-ahead log, snapshot+diff WebSocket, and >5M order-ops/sec throughput.",
  },
  {
    icon: BarChart2,
    title: "Real market data",
    body: "Binance live feed powers the order book seed. Kline history streamed to lightweight-charts.",
  },
  {
    icon: Shield,
    title: "Production security",
    body: "Argon2id passwords, JWT rotation, HMAC API keys, TOTP 2FA, Redis rate limiting, and full security headers.",
  },
] as const;

const MARKETS = [
  { symbol: "BTC/USDT", href: "/trade/BTCUSDT" },
  { symbol: "ETH/USDT", href: "/trade/ETHUSDT" },
  { symbol: "SOL/USDT", href: "/trade/SOLUSDT" },
] as const;

export default function LandingPage() {
  return (
    <div className="flex min-h-screen flex-col bg-zinc-950 text-zinc-100">
      {/* Nav */}
      <header className="flex h-14 items-center justify-between border-b border-zinc-900 px-6">
        <div className="flex items-center gap-2 font-bold">
          <span className="text-emerald-400">⬡</span>
          <span className="text-sm tracking-wide">AETHER</span>
        </div>
        <div className="flex items-center gap-2">
          <Link
            href="/auth/login"
            className="rounded px-3 py-1.5 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
          >
            Sign in
          </Link>
          <Link
            href="/auth/signup"
            className="rounded bg-emerald-500 px-3 py-1.5 text-xs font-medium text-zinc-950 hover:bg-emerald-400 transition-colors"
          >
            Get started
          </Link>
        </div>
      </header>

      {/* Hero */}
      <section className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-20 text-center">
        <div className="inline-flex items-center gap-2 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-3 py-1 text-xs text-emerald-400">
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
          Portfolio demo — production architecture
        </div>

        <h1 className="max-w-2xl text-4xl font-bold leading-tight tracking-tight sm:text-5xl">
          A crypto exchange built for{" "}
          <span className="text-emerald-400">engineering credibility</span>
        </h1>

        <p className="max-w-xl text-base text-zinc-400 leading-7">
          Rust matching engine, Next.js 16 trading UI, double-entry ledger, snapshot+diff WebSocket
          protocol, and zero floats in the money path — all open source.
        </p>

        <div className="flex flex-wrap gap-3 justify-center">
          <Link
            href="/trade/BTCUSDT"
            className="flex items-center gap-2 rounded-lg bg-emerald-500 px-5 py-2.5 text-sm font-medium text-zinc-950 hover:bg-emerald-400 transition-colors"
          >
            Open trading screen
            <ArrowRight className="h-4 w-4" aria-hidden />
          </Link>
          <Link
            href="/markets"
            className="flex items-center gap-2 rounded-lg border border-zinc-700 px-5 py-2.5 text-sm font-medium text-zinc-300 hover:border-zinc-600 hover:bg-zinc-900 transition-colors"
          >
            View all markets
          </Link>
        </div>
      </section>

      {/* Quick-trade links */}
      <section className="border-y border-zinc-900 py-5">
        <div className="mx-auto flex max-w-3xl flex-wrap items-center justify-center gap-3 px-6">
          {MARKETS.map(({ symbol, href }) => (
            <Link
              key={symbol}
              href={href}
              className="rounded-lg border border-zinc-800 bg-zinc-900 px-4 py-2 text-sm font-medium text-zinc-300 hover:border-zinc-700 hover:bg-zinc-800 hover:text-zinc-100 transition-colors"
            >
              {symbol}
            </Link>
          ))}
        </div>
      </section>

      {/* Feature cards */}
      <section className="mx-auto grid max-w-4xl gap-4 px-6 py-16 sm:grid-cols-3">
        {FEATURES.map(({ icon: Icon, title, body }) => (
          <div
            key={title}
            className="flex flex-col gap-3 rounded-xl border border-zinc-800 bg-zinc-900 p-5"
          >
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-500/10">
              <Icon className="h-4.5 w-4.5 text-emerald-400" aria-hidden />
            </div>
            <h3 className="font-semibold text-zinc-100 text-sm">{title}</h3>
            <p className="text-xs text-zinc-400 leading-5">{body}</p>
          </div>
        ))}
      </section>

      {/* Footer */}
      <footer className="border-t border-zinc-900 py-6 text-center text-xs text-zinc-600">
        Aether is a portfolio demonstration. It does not custody real funds.{" "}
        <Link href="https://github.com/dhruvxvaishnav" className="underline hover:text-zinc-400">
          GitHub
        </Link>
      </footer>
    </div>
  );
}

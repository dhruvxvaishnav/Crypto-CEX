const MARKET_ROWS = [
  ["BTC/USDT", "68,420.12", "+2.14%", "1.42B"],
  ["ETH/USDT", "3,780.44", "+1.02%", "864M"],
  ["SOL/USDT", "184.92", "-0.48%", "226M"],
] as const;

const SYSTEM_SIGNALS = [
  ["engine", "ready"],
  ["settlement", "idle"],
  ["market data", "warming"],
] as const;

export default function Home() {
  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100">
      <section className="mx-auto flex min-h-screen w-full max-w-7xl flex-col px-5 py-5 sm:px-8">
        <header className="flex items-center justify-between border-neutral-800 border-b pb-4">
          <div>
            <p className="text-neutral-400 text-sm">Aether</p>
            <h1 className="font-semibold text-2xl text-white">Spot trading console</h1>
          </div>
          <nav
            aria-label="Primary"
            className="hidden items-center gap-6 text-neutral-300 text-sm md:flex"
          >
            <a href="/markets">Markets</a>
            <a href="/trade/BTCUSDT">Trade</a>
            <a href="/portfolio">Portfolio</a>
            <a href="/proof-of-reserves">Proof</a>
          </nav>
        </header>

        <div className="grid flex-1 gap-5 py-5 lg:grid-cols-[1.2fr_0.8fr]">
          <section className="flex min-h-[520px] flex-col justify-between border-neutral-800 border-r pr-0 lg:pr-6">
            <div>
              <p className="mb-3 text-emerald-300 text-sm">Demo exchange foundation</p>
              <h2 className="max-w-3xl font-semibold text-4xl text-white leading-tight">
                Production-shaped crypto exchange architecture, ready for the matching engine.
              </h2>
              <p className="mt-5 max-w-2xl text-base text-neutral-300 leading-7">
                Rust engine workspace, latest Next.js frontend, shared contracts, database schema,
                local infrastructure, and CI are now the baseline for feature work.
              </p>
            </div>

            <div className="grid gap-3 md:grid-cols-3">
              {SYSTEM_SIGNALS.map(([label, value]) => (
                <div key={label} className="border border-neutral-800 p-4">
                  <p className="text-neutral-500 text-xs uppercase">{label}</p>
                  <p className="mt-2 font-medium text-lg text-white">{value}</p>
                </div>
              ))}
            </div>
          </section>

          <section className="flex flex-col gap-5">
            <div className="border border-neutral-800">
              <div className="grid grid-cols-4 border-neutral-800 border-b px-4 py-3 text-neutral-500 text-xs uppercase">
                <span>Market</span>
                <span className="text-right">Last</span>
                <span className="text-right">24h</span>
                <span className="text-right">Volume</span>
              </div>
              {MARKET_ROWS.map(([symbol, last, change, volume]) => {
                const changeTone = change.startsWith("-") ? "text-rose-300" : "text-emerald-300";

                return (
                  <div
                    className="grid grid-cols-4 border-neutral-900 border-b px-4 py-4 text-sm last:border-b-0"
                    key={symbol}
                  >
                    <span className="font-medium text-white">{symbol}</span>
                    <span className="text-right text-neutral-200">{last}</span>
                    <span className={`text-right ${changeTone}`}>{change}</span>
                    <span className="text-right text-neutral-300">{volume}</span>
                  </div>
                );
              })}
            </div>

            <div className="border border-neutral-800 p-4">
              <p className="text-neutral-500 text-xs uppercase">Build status</p>
              <p className="mt-2 text-neutral-200 text-sm leading-6">
                Day 1 foundation is being established. Trading flows remain disabled until the
                engine, API, and settlement services land.
              </p>
            </div>
          </section>
        </div>

        <footer className="border-neutral-800 border-t pt-4 text-neutral-500 text-sm">
          Aether is a portfolio demonstration and demo exchange. It does not custody real funds.
        </footer>
      </section>
    </main>
  );
}

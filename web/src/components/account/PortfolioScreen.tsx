"use client";

import { useQuery } from "@tanstack/react-query";
import { Activity, CircleDollarSign, LockKeyhole, TrendingUp, WalletCards } from "lucide-react";
import type { ReactNode } from "react";
import { useMemo } from "react";

import { AccountShell } from "@/components/account/AccountShell";
import {
  estimatePortfolioValue,
  formatDecimal,
  formatSignedDecimal,
} from "@/components/account/account-model";
import { AsyncBoundary } from "@/components/ui/AsyncBoundary";
import { getBalances, getLedgerHistory, getPnl } from "@/lib/account-api";
import { getMarkets } from "@/lib/market-data";
import type { Balance, LedgerEntry, PnlEntry } from "@/types/api.types";

const TIME_FORMATTER = new Intl.DateTimeFormat("en", {
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  month: "short",
  timeZone: "UTC",
});

export function PortfolioScreen() {
  const balancesQuery = useQuery({ queryKey: ["balances"], queryFn: getBalances });
  const marketsQuery = useQuery({ queryKey: ["markets"], queryFn: getMarkets });
  const historyQuery = useQuery({
    queryKey: ["ledger-history", "portfolio"],
    queryFn: () => getLedgerHistory({ limit: 80 }),
  });
  const pnlQuery = useQuery({ queryKey: ["account-pnl"], queryFn: getPnl });

  const balances = balancesQuery.data ?? [];
  const markets = marketsQuery.data ?? [];
  const estimate = useMemo(() => estimatePortfolioValue(balances, markets), [balances, markets]);
  const nonZeroBalances = useMemo(
    () =>
      balances.filter(
        (balance) => balance.total !== "0" && balance.total !== "0.000000000000000000",
      ),
    [balances],
  );
  const lockedAssetCount = useMemo(
    () =>
      balances.filter(
        (balance) => balance.locked !== "0" && balance.locked !== "0.000000000000000000",
      ).length,
    [balances],
  );

  return (
    <AccountShell>
      <header className="flex flex-col gap-1">
        <h1 className="text-xl font-semibold text-zinc-100">Portfolio</h1>
        <p className="text-sm text-zinc-500">Balances, valuation, and ledger activity.</p>
      </header>

      <section className="grid gap-3 md:grid-cols-4" aria-label="Portfolio summary">
        <SummaryTile
          icon={<CircleDollarSign className="h-4 w-4" aria-hidden />}
          label="Estimated value"
          value={`$${estimate.totalUsd}`}
        />
        <SummaryTile
          icon={<WalletCards className="h-4 w-4" aria-hidden />}
          label="Priced assets"
          value={estimate.pricedAssetCount.toString()}
        />
        <SummaryTile
          icon={<Activity className="h-4 w-4" aria-hidden />}
          label="Assets held"
          value={nonZeroBalances.length.toString()}
        />
        <SummaryTile
          icon={<LockKeyhole className="h-4 w-4" aria-hidden />}
          label="Locked assets"
          value={lockedAssetCount.toString()}
        />
      </section>

      <section className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(360px,0.9fr)]">
        <Panel title="Balances">
          <AsyncBoundary
            isLoading={balancesQuery.isLoading}
            isEmpty={!balancesQuery.isLoading && balances.length === 0}
          >
            <BalancesTable balances={balances} />
          </AsyncBoundary>
        </Panel>

        <Panel title="Ledger">
          <AsyncBoundary
            isLoading={historyQuery.isLoading}
            isEmpty={!historyQuery.isLoading && (historyQuery.data?.data.length ?? 0) === 0}
          >
            <LedgerTable entries={historyQuery.data?.data ?? []} />
          </AsyncBoundary>
        </Panel>
      </section>

      <Panel title="FIFO P&L">
        <AsyncBoundary
          isLoading={pnlQuery.isLoading}
          isEmpty={!pnlQuery.isLoading && (pnlQuery.data?.length ?? 0) === 0}
        >
          <PnlTable rows={pnlQuery.data ?? []} />
        </AsyncBoundary>
      </Panel>
    </AccountShell>
  );
}

function SummaryTile({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-900/40 p-4">
      <div className="mb-3 flex h-7 w-7 items-center justify-center rounded bg-zinc-800 text-emerald-300">
        {icon}
      </div>
      <p className="text-xs text-zinc-500">{label}</p>
      <p className="mt-1 text-lg font-semibold text-zinc-100">{value}</p>
    </div>
  );
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="min-h-[360px] overflow-hidden rounded-md border border-zinc-800 bg-zinc-950">
      <div className="flex h-11 items-center border-b border-zinc-800 px-4">
        <h2 className="text-sm font-medium text-zinc-100">{title}</h2>
      </div>
      {children}
    </section>
  );
}

function BalancesTable({ balances }: { balances: Balance[] }) {
  return (
    <div className="overflow-auto">
      <div className="grid min-w-[720px] grid-cols-[110px_1fr_1fr_1fr_1fr] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
        <span>Asset</span>
        <span>Name</span>
        <span className="text-right">Available</span>
        <span className="text-right">Locked</span>
        <span className="text-right">Total</span>
      </div>
      {balances.map((balance) => (
        <div
          key={balance.asset}
          className="grid min-w-[720px] grid-cols-[110px_1fr_1fr_1fr_1fr] px-4 text-sm leading-10 hover:bg-zinc-900/70"
        >
          <span className="font-medium text-zinc-100">{balance.asset}</span>
          <span className="truncate text-zinc-500">{balance.assetName}</span>
          <span className="text-right text-zinc-300">{formatDecimal(balance.available)}</span>
          <span className="text-right text-amber-300">{formatDecimal(balance.locked)}</span>
          <span className="text-right text-zinc-100">{formatDecimal(balance.total)}</span>
        </div>
      ))}
    </div>
  );
}

function LedgerTable({ entries }: { entries: LedgerEntry[] }) {
  return (
    <div className="overflow-auto">
      <div className="grid min-w-[620px] grid-cols-[110px_100px_90px_1fr_1fr] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
        <span>Time</span>
        <span>Kind</span>
        <span>Asset</span>
        <span className="text-right">Amount</span>
        <span className="text-right">Available after</span>
      </div>
      {entries.map((entry) => (
        <div
          key={entry.id}
          className="grid min-w-[620px] grid-cols-[110px_100px_90px_1fr_1fr] px-4 text-sm leading-10 hover:bg-zinc-900/70"
        >
          <span className="text-zinc-500">{formatTime(entry.ts)}</span>
          <span className="capitalize text-zinc-300">{entry.kind}</span>
          <span className="text-zinc-400">{entry.asset}</span>
          <span className="text-right text-zinc-100">{formatDecimal(entry.amount)}</span>
          <span className="text-right text-zinc-500">{formatDecimal(entry.availableAfter)}</span>
        </div>
      ))}
    </div>
  );
}

function PnlTable({ rows }: { rows: PnlEntry[] }) {
  return (
    <div className="overflow-auto">
      <div className="grid min-w-[780px] grid-cols-[100px_1fr_1fr_1fr_1fr_1fr] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
        <span>Asset</span>
        <span className="text-right">Qty</span>
        <span className="text-right">Avg cost</span>
        <span className="text-right">Market</span>
        <span className="text-right">Unrealised</span>
        <span className="text-right">Realised</span>
      </div>
      {rows.map((row) => (
        <div
          key={row.asset}
          className="grid min-w-[780px] grid-cols-[100px_1fr_1fr_1fr_1fr_1fr] px-4 text-sm leading-10 hover:bg-zinc-900/70"
        >
          <span className="flex items-center gap-2 font-medium text-zinc-100">
            <TrendingUp className="h-3.5 w-3.5 text-emerald-300" aria-hidden />
            {row.asset}
          </span>
          <span className="text-right text-zinc-300">{formatDecimal(row.qty)}</span>
          <span className="text-right text-zinc-500">{formatDecimal(row.avgCost)}</span>
          <span className="text-right text-zinc-300">
            {row.marketPrice ? formatDecimal(row.marketPrice) : "--"}
          </span>
          <PnlCell value={row.unrealisedPnl} />
          <PnlCell value={row.realisedPnl} />
        </div>
      ))}
    </div>
  );
}

function PnlCell({ value }: { value: string | null }) {
  if (!value) return <span className="text-right text-zinc-600">--</span>;
  const isLoss = value.startsWith("-");
  const classes = isLoss ? "text-rose-300" : "text-emerald-300";
  return <span className={`text-right ${classes}`}>{formatSignedDecimal(value)}</span>;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--";
  return TIME_FORMATTER.format(date);
}

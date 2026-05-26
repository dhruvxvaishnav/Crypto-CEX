"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Ban, Lock, PauseCircle, PlayCircle, RefreshCw, ShieldAlert } from "lucide-react";
import type { ReactNode } from "react";
import { useMemo, useState } from "react";
import type {
  ActionMessage,
  AdminActionSummaryProps,
  EngineStateTableProps,
  FreezeUserPanelProps,
  MarketControlsProps,
} from "@/components/admin/AdminScreen.types";
import { statusTone, topOfBookText, totalOpenOrders } from "@/components/admin/admin-model";
import { TopBar } from "@/components/layout/TopBar";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { getProfile } from "@/lib/account-api";
import {
  cancelAllInMarket,
  freezeUser,
  getAdminEngineState,
  haltMarket,
  resumeMarket,
} from "@/lib/admin-api";
import { ApiError } from "@/lib/api-client";
import type { AdminActionResponse } from "@/types/api.types";

export function AdminScreen() {
  const queryClient = useQueryClient();
  const [message, setMessage] = useState<ActionMessage | null>(null);
  const [lastAction, setLastAction] = useState<AdminActionResponse | null>(null);
  const [pendingAction, setPendingAction] = useState<string | null>(null);
  const profileQuery = useQuery({ queryKey: ["profile"], queryFn: getProfile });
  const engineStateQuery = useQuery({
    queryKey: ["admin-engine-state"],
    queryFn: getAdminEngineState,
    enabled: profileQuery.data?.isAdmin === true,
    refetchInterval: 5000,
  });
  const markets = engineStateQuery.data?.markets ?? [];
  const openOrders = useMemo(() => totalOpenOrders(markets), [markets]);

  const actionMutation = useMutation({
    mutationFn: runAdminAction,
    onMutate(input) {
      setPendingAction(actionKey(input));
      setMessage(null);
    },
    onSuccess(data) {
      setLastAction(data);
      setMessage({ tone: "success", text: "Admin command completed" });
      void queryClient.invalidateQueries({ queryKey: ["admin-engine-state"] });
    },
    onError(error) {
      setMessage({
        tone: "danger",
        text: error instanceof ApiError ? error.message : "Admin command failed",
      });
    },
    onSettled() {
      setPendingAction(null);
    },
  });

  if (profileQuery.isLoading) {
    return <AdminShell>Loading admin profile...</AdminShell>;
  }

  if (!profileQuery.data?.isAdmin) {
    return (
      <AdminShell>
        <Panel title="Access denied" icon={<ShieldAlert className="h-4 w-4" aria-hidden />}>
          <p className="text-sm text-zinc-500">Admin privileges are required for this console.</p>
        </Panel>
      </AdminShell>
    );
  }

  return (
    <AdminShell>
      <header className="flex flex-col gap-1">
        <h1 className="text-xl font-semibold text-zinc-100">Admin Console</h1>
        <p className="text-sm text-zinc-500">Market controls, user freezes, and engine state.</p>
      </header>

      {message && <Alert message={message} />}

      <section className="grid gap-3 md:grid-cols-3">
        <Metric label="Markets" value={markets.length.toString()} />
        <Metric label="Open orders" value={openOrders.toString()} />
        <Metric label="State refresh" value={engineStateQuery.isFetching ? "Syncing" : "Live"} />
      </section>

      <section className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
        <Panel title="Market controls" icon={<PauseCircle className="h-4 w-4" aria-hidden />}>
          <div className="grid gap-3">
            {markets.map((market) => (
              <MarketControls
                key={market.symbol}
                market={market}
                onCancelAll={(symbol) => actionMutation.mutate({ action: "cancelAll", symbol })}
                onHalt={(symbol) => actionMutation.mutate({ action: "halt", symbol })}
                onResume={(symbol) => actionMutation.mutate({ action: "resume", symbol })}
                pendingAction={pendingAction}
              />
            ))}
            {markets.length === 0 && (
              <p className="text-sm text-zinc-500">No markets are available to administer.</p>
            )}
          </div>
        </Panel>

        <div className="flex flex-col gap-4">
          <FreezeUserPanel
            isPending={pendingAction?.startsWith("freeze:") ?? false}
            onFreeze={(userId) => actionMutation.mutate({ action: "freeze", userId })}
          />
          <AdminActionSummary response={lastAction} />
        </div>
      </section>

      <Panel title="Engine state" icon={<RefreshCw className="h-4 w-4" aria-hidden />}>
        <EngineStateTable markets={markets} />
      </Panel>
    </AdminShell>
  );
}

function AdminShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col bg-zinc-950 text-zinc-100">
      <TopBar />
      <main
        id="main-content"
        className="mx-auto flex w-full max-w-7xl flex-1 flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
      >
        {typeof children === "string" ? (
          <p className="text-sm text-zinc-500">{children}</p>
        ) : (
          children
        )}
      </main>
    </div>
  );
}

function MarketControls({
  market,
  onCancelAll,
  onHalt,
  onResume,
  pendingAction,
}: MarketControlsProps) {
  const tone = statusTone(market.status);
  const toneClass = {
    danger: "border-rose-900/70 bg-rose-950/20 text-rose-300",
    muted: "border-zinc-800 bg-zinc-900 text-zinc-500",
    success: "border-emerald-900/70 bg-emerald-950/20 text-emerald-300",
  }[tone];

  return (
    <article className="grid gap-3 rounded-md border border-zinc-800 bg-zinc-900/40 p-3 lg:grid-cols-[1fr_auto] lg:items-center">
      <div>
        <div className="mb-2 flex items-center gap-2">
          <h3 className="text-sm font-medium text-zinc-100">{market.symbol}</h3>
          <span className={`rounded border px-2 py-0.5 text-xs ${toneClass}`}>{market.status}</span>
        </div>
        <p className="text-xs text-zinc-500">
          Open orders {market.openOrderCount} · bid {topOfBookText(market.bestBid)} · ask{" "}
          {topOfBookText(market.bestAsk)}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        {market.status === "halted" ? (
          <Button
            type="button"
            size="sm"
            onClick={() => onResume(market.symbol)}
            isLoading={pendingAction === `resume:${market.symbol}`}
          >
            <PlayCircle className="h-3.5 w-3.5" aria-hidden />
            Resume
          </Button>
        ) : (
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => onHalt(market.symbol)}
            isLoading={pendingAction === `halt:${market.symbol}`}
          >
            <PauseCircle className="h-3.5 w-3.5" aria-hidden />
            Halt
          </Button>
        )}
        <Button
          type="button"
          size="sm"
          variant="danger"
          onClick={() => onCancelAll(market.symbol)}
          isLoading={pendingAction === `cancelAll:${market.symbol}`}
        >
          <Ban className="h-3.5 w-3.5" aria-hidden />
          Cancel all
        </Button>
      </div>
    </article>
  );
}

function FreezeUserPanel({ isPending, onFreeze }: FreezeUserPanelProps) {
  const [userId, setUserId] = useState("");

  function submit() {
    const trimmed = userId.trim();
    if (trimmed !== "") onFreeze(trimmed);
  }

  return (
    <Panel title="Freeze user" icon={<Lock className="h-4 w-4" aria-hidden />}>
      <div className="flex flex-col gap-3">
        <Input
          label="User ID"
          placeholder="UUID"
          value={userId}
          onChange={(event) => setUserId(event.currentTarget.value)}
        />
        <Button type="button" variant="danger" isLoading={isPending} onClick={submit}>
          Freeze user
        </Button>
      </div>
    </Panel>
  );
}

function AdminActionSummary({ response }: AdminActionSummaryProps) {
  return (
    <Panel title="Last action" icon={<ShieldAlert className="h-4 w-4" aria-hidden />}>
      {response ? (
        <div className="grid gap-2 text-sm">
          <SummaryRow label="Symbol" value={response.symbol ?? "--"} />
          <SummaryRow label="Status" value={response.status ?? "--"} />
          <SummaryRow label="Canceled" value={response.count.toString()} />
        </div>
      ) : (
        <p className="text-sm text-zinc-500">No admin command has run in this session.</p>
      )}
    </Panel>
  );
}

function EngineStateTable({ markets }: EngineStateTableProps) {
  return (
    <div className="overflow-auto">
      <div className="grid min-w-[760px] grid-cols-[120px_100px_120px_1fr_1fr_100px] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
        <span>Market</span>
        <span>Status</span>
        <span className="text-right">Open orders</span>
        <span className="text-right">Best bid</span>
        <span className="text-right">Best ask</span>
        <span className="text-right">Seq</span>
      </div>
      {markets.map((market) => (
        <div
          key={market.symbol}
          className="grid min-w-[760px] grid-cols-[120px_100px_120px_1fr_1fr_100px] px-4 text-sm leading-10 hover:bg-zinc-900/70"
        >
          <span className="font-medium text-zinc-100">{market.symbol}</span>
          <span className="text-zinc-400">{market.status}</span>
          <span className="text-right text-zinc-300">{market.openOrderCount}</span>
          <span className="text-right text-emerald-300">{topOfBookText(market.bestBid)}</span>
          <span className="text-right text-rose-300">{topOfBookText(market.bestAsk)}</span>
          <span className="text-right text-zinc-500">{market.seq ?? "--"}</span>
        </div>
      ))}
    </div>
  );
}

function Panel({ children, icon, title }: { children: ReactNode; icon: ReactNode; title: string }) {
  return (
    <section className="overflow-hidden rounded-md border border-zinc-800 bg-zinc-950">
      <div className="flex h-11 items-center gap-2 border-b border-zinc-800 px-4">
        <span className="text-emerald-300">{icon}</span>
        <h2 className="text-sm font-medium text-zinc-100">{title}</h2>
      </div>
      <div className="p-4">{children}</div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-900/40 p-4">
      <p className="text-xs text-zinc-500">{label}</p>
      <p className="mt-1 text-lg font-semibold text-zinc-100">{value}</p>
    </div>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-zinc-500">{label}</span>
      <span className="truncate text-zinc-100">{value}</span>
    </div>
  );
}

function Alert({ message }: { message: ActionMessage }) {
  const classes =
    message.tone === "danger"
      ? "border-rose-900/70 bg-rose-950/20 text-rose-300"
      : "border-emerald-900/70 bg-emerald-950/20 text-emerald-300";
  return <p className={`rounded-md border px-3 py-2 text-sm ${classes}`}>{message.text}</p>;
}

type AdminMutationInput =
  | { action: "cancelAll"; symbol: string }
  | { action: "freeze"; userId: string }
  | { action: "halt"; symbol: string }
  | { action: "resume"; symbol: string };

function actionKey(input: AdminMutationInput): string {
  return input.action === "freeze"
    ? `${input.action}:${input.userId}`
    : `${input.action}:${input.symbol}`;
}

function runAdminAction(input: AdminMutationInput): Promise<AdminActionResponse> {
  switch (input.action) {
    case "cancelAll":
      return cancelAllInMarket(input.symbol);
    case "freeze":
      return freezeUser(input.userId);
    case "halt":
      return haltMarket(input.symbol);
    case "resume":
      return resumeMarket(input.symbol);
  }
}

"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BadgeDollarSign, Send, ShieldAlert } from "lucide-react";
import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { AccountShell } from "@/components/account/AccountShell";
import {
  formatDecimal,
  getWithdrawMinimum,
  validateFaucetInput,
  validateWithdrawInput,
} from "@/components/account/account-model";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { getBalances, getLedgerHistory, requestFaucet } from "@/lib/account-api";
import { ApiError } from "@/lib/api-client";
import { zodResolver } from "@/lib/zod-resolver";
import type { Balance, LedgerEntry } from "@/types/api.types";

const FAUCET_SCHEMA = z
  .object({
    asset: z.string().min(1, "Asset is required"),
    amount: z.string(),
  })
  .superRefine((values, ctx) => {
    const error = validateFaucetInput(values.asset, values.amount);
    if (error) {
      ctx.addIssue({ code: "custom", path: ["amount"], message: error });
    }
  });

const WITHDRAW_SCHEMA = z
  .object({
    asset: z.string().min(1, "Asset is required"),
    amount: z.string(),
    address: z.string(),
  })
  .superRefine((values, ctx) => {
    const result = validateWithdrawInput(values);
    if (result.amountError) {
      ctx.addIssue({ code: "custom", path: ["amount"], message: result.amountError });
    }
    if (result.addressError) {
      ctx.addIssue({ code: "custom", path: ["address"], message: result.addressError });
    }
  });

type FaucetValues = z.infer<typeof FAUCET_SCHEMA>;
type WithdrawValues = z.infer<typeof WITHDRAW_SCHEMA>;

const TIME_FORMATTER = new Intl.DateTimeFormat("en", {
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  month: "short",
  timeZone: "UTC",
});

export function WalletScreen() {
  const queryClient = useQueryClient();
  const [serverError, setServerError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const balancesQuery = useQuery({ queryKey: ["balances"], queryFn: getBalances });
  const historyQuery = useQuery({
    queryKey: ["ledger-history", "wallet"],
    queryFn: () => getLedgerHistory({ limit: 80 }),
  });
  const balances = balancesQuery.data ?? [];
  const assets = useMemo(() => balances.map((balance) => balance.asset), [balances]);
  const defaultAsset = assets[0] ?? "USDT";

  const faucetForm = useForm<FaucetValues>({
    resolver: zodResolver(FAUCET_SCHEMA),
    values: { asset: defaultAsset, amount: "" },
  });
  const withdrawForm = useForm<WithdrawValues>({
    resolver: zodResolver(WITHDRAW_SCHEMA),
    values: { asset: defaultAsset, amount: "", address: "" },
  });
  const withdrawAsset = withdrawForm.watch("asset");
  const withdrawMinimum = getWithdrawMinimum(withdrawAsset);

  const faucetMutation = useMutation({
    mutationFn: requestFaucet,
    onSuccess(data) {
      setServerError(null);
      setSuccessMessage(`Credited ${formatDecimal(data.amount)} ${data.asset}`);
      faucetForm.reset({ asset: data.asset, amount: "" });
      void queryClient.invalidateQueries({ queryKey: ["balances"] });
      void queryClient.invalidateQueries({ queryKey: ["ledger-history"] });
    },
    onError(error) {
      setSuccessMessage(null);
      if (error instanceof ApiError) {
        setServerError(error.message);
      } else {
        setServerError("Faucet request failed");
      }
    },
  });

  function submitFaucet(values: FaucetValues) {
    setServerError(null);
    setSuccessMessage(null);
    faucetMutation.mutate({ asset: values.asset, amount: values.amount.trim() });
  }

  return (
    <AccountShell>
      <header className="flex flex-col gap-1">
        <h1 className="text-xl font-semibold text-zinc-100">Wallet</h1>
        <p className="text-sm text-zinc-500">Demo funding, withdrawal checks, and transfers.</p>
      </header>

      <section className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
        <div className="flex flex-col gap-4">
          <section className="rounded-md border border-zinc-800 bg-zinc-950 p-4">
            <div className="mb-4 flex items-center gap-2">
              <BadgeDollarSign className="h-4 w-4 text-emerald-300" aria-hidden />
              <h2 className="text-sm font-medium text-zinc-100">Demo faucet</h2>
            </div>
            <div className="mb-4 rounded-md border border-emerald-900/70 bg-emerald-950/20 px-3 py-2 text-xs text-emerald-200">
              Demo faucet - testnet credits
            </div>
            <form className="flex flex-col gap-3" onSubmit={faucetForm.handleSubmit(submitFaucet)}>
              <AssetSelect
                assets={assets}
                value={faucetForm.watch("asset")}
                onChange={(value) => faucetForm.setValue("asset", value, { shouldValidate: true })}
              />
              <Input
                label="Amount"
                inputMode="decimal"
                placeholder="1000"
                error={faucetForm.formState.errors.amount?.message}
                {...faucetForm.register("amount")}
              />
              {serverError && <Alert tone="danger">{serverError}</Alert>}
              {successMessage && <Alert tone="success">{successMessage}</Alert>}
              <Button type="submit" isLoading={faucetMutation.isPending}>
                Credit wallet
              </Button>
            </form>
          </section>

          <section className="rounded-md border border-zinc-800 bg-zinc-950 p-4">
            <div className="mb-4 flex items-center gap-2">
              <Send className="h-4 w-4 text-zinc-300" aria-hidden />
              <h2 className="text-sm font-medium text-zinc-100">Withdraw</h2>
            </div>
            <form className="flex flex-col gap-3">
              <AssetSelect
                assets={assets}
                value={withdrawAsset}
                onChange={(value) =>
                  withdrawForm.setValue("asset", value, { shouldValidate: true })
                }
              />
              <Input
                label="Amount"
                inputMode="decimal"
                placeholder={withdrawMinimum ? `Min ${withdrawMinimum}` : "0"}
                error={withdrawForm.formState.errors.amount?.message}
                {...withdrawForm.register("amount")}
              />
              <Input
                label="Address"
                placeholder="Destination address"
                error={withdrawForm.formState.errors.address?.message}
                {...withdrawForm.register("address")}
              />
              <div className="rounded-md border border-amber-900/70 bg-amber-950/20 px-3 py-2 text-xs text-amber-200">
                Simulated withdrawals require the backend withdrawal endpoint before submission.
              </div>
              <Button type="button" variant="secondary" disabled>
                Withdraw unavailable
              </Button>
            </form>
          </section>
        </div>

        <section className="overflow-hidden rounded-md border border-zinc-800 bg-zinc-950">
          <div className="flex h-11 items-center justify-between border-b border-zinc-800 px-4">
            <h2 className="text-sm font-medium text-zinc-100">Wallet activity</h2>
            <ShieldAlert className="h-4 w-4 text-zinc-600" aria-hidden />
          </div>
          <WalletActivity
            balances={balances}
            entries={historyQuery.data?.data ?? []}
            isLoading={balancesQuery.isLoading || historyQuery.isLoading}
          />
        </section>
      </section>
    </AccountShell>
  );
}

function AssetSelect({
  assets,
  value,
  onChange,
}: {
  assets: string[];
  value: string;
  onChange: (value: string) => void;
}) {
  const options = assets.length > 0 ? assets : ["USDT"];
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs font-medium uppercase tracking-wide text-zinc-400">Asset</span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="h-9 rounded-md border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 focus:outline-none focus:ring-2 focus:ring-emerald-500"
      >
        {options.map((asset) => (
          <option key={asset} value={asset}>
            {asset}
          </option>
        ))}
      </select>
    </label>
  );
}

function WalletActivity({
  balances,
  entries,
  isLoading,
}: {
  balances: Balance[];
  entries: LedgerEntry[];
  isLoading: boolean;
}) {
  if (isLoading) {
    return <div className="p-4 text-sm text-zinc-500">Loading wallet activity...</div>;
  }

  return (
    <div className="grid min-h-[520px] lg:grid-cols-[280px_minmax(0,1fr)]">
      <div className="border-b border-zinc-800 p-4 lg:border-r lg:border-b-0">
        <h3 className="mb-3 text-xs font-medium uppercase tracking-wide text-zinc-500">Balances</h3>
        <div className="flex flex-col gap-2">
          {balances.map((balance) => (
            <div key={balance.asset} className="rounded border border-zinc-800 bg-zinc-900/50 p-3">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-zinc-100">{balance.asset}</span>
                <span className="text-xs text-zinc-500">{balance.assetName}</span>
              </div>
              <p className="mt-2 text-sm text-zinc-300">{formatDecimal(balance.available)}</p>
              <p className="mt-1 text-xs text-zinc-600">Locked {formatDecimal(balance.locked)}</p>
            </div>
          ))}
        </div>
      </div>
      <div className="overflow-auto">
        <div className="grid min-w-[640px] grid-cols-[120px_110px_90px_1fr_1fr] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
          <span>Time</span>
          <span>Kind</span>
          <span>Asset</span>
          <span className="text-right">Amount</span>
          <span className="text-right">Reference</span>
        </div>
        {entries.map((entry) => (
          <div
            key={entry.id}
            className="grid min-w-[640px] grid-cols-[120px_110px_90px_1fr_1fr] px-4 text-sm leading-10 hover:bg-zinc-900/70"
          >
            <span className="text-zinc-500">{formatTime(entry.ts)}</span>
            <span className="capitalize text-zinc-300">{entry.kind}</span>
            <span className="text-zinc-400">{entry.asset}</span>
            <span className="text-right text-zinc-100">{formatDecimal(entry.amount)}</span>
            <span className="truncate text-right text-zinc-600">{entry.referenceType}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Alert({ tone, children }: { tone: "danger" | "success"; children: string }) {
  const classes =
    tone === "danger"
      ? "border-rose-800 bg-rose-950/60 text-rose-300"
      : "border-emerald-800 bg-emerald-950/50 text-emerald-300";
  return <p className={`rounded-md border px-3 py-2 text-xs ${classes}`}>{children}</p>;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--";
  return TIME_FORMATTER.format(date);
}

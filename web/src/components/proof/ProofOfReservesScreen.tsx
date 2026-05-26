"use client";

import { useMutation, useQuery } from "@tanstack/react-query";
import { Database, KeyRound, RefreshCw, ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";

import { formatDecimal } from "@/components/account/account-model";
import { TopBar } from "@/components/layout/TopBar";
import { Button } from "@/components/ui/Button";
import { ApiError } from "@/lib/api-client";
import { getLatestProof, getMyProof } from "@/lib/proof-api";
import type { MyProof, ProofEntry, ProofLatest } from "@/types/api.types";

const ROOT_PREVIEW = 18;
const VERIFY_SNIPPET = `const leaf = sha256(leafPayload);
const root = siblingHashes.reduce((hash, sibling) => {
  const [left, right] = hash <= sibling ? [hash, sibling] : [sibling, hash];
  return sha256(\`aether:por:v1:node:\${left}:\${right}\`);
}, leaf);
console.assert(root === merkleRoot);`;

export function ProofOfReservesScreen() {
  const latestQuery = useQuery({ queryKey: ["proof-latest"], queryFn: getLatestProof });
  const myProofMutation = useMutation({ mutationFn: getMyProof });

  return (
    <div className="flex min-h-screen flex-col bg-zinc-950 text-zinc-100">
      <TopBar />
      <main className="mx-auto flex w-full max-w-7xl flex-1 flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8">
        <header className="flex flex-col gap-1">
          <h1 className="text-xl font-semibold text-zinc-100">Proof of Reserves</h1>
          <p className="text-sm text-zinc-500">
            Merkle-rooted customer liabilities generated from the live ledger.
          </p>
        </header>

        {latestQuery.isLoading ? (
          <Panel title="Snapshot">Loading proof snapshot...</Panel>
        ) : latestQuery.data ? (
          <SnapshotView
            latest={latestQuery.data}
            onVerify={() => myProofMutation.mutate()}
            isVerifying={myProofMutation.isPending}
          />
        ) : (
          <Panel title="Snapshot">Proof snapshot is unavailable.</Panel>
        )}

        {myProofMutation.data && <MyProofView proof={myProofMutation.data} />}
        {myProofMutation.error && <VerifyError error={myProofMutation.error} />}
      </main>
    </div>
  );
}

function SnapshotView({
  latest,
  onVerify,
  isVerifying,
}: {
  latest: ProofLatest;
  onVerify: () => void;
  isVerifying: boolean;
}) {
  return (
    <section className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(420px,1.1fr)]">
      <Panel title="Committed root">
        <div className="flex flex-col gap-4 p-4">
          <div className="grid gap-3 sm:grid-cols-3">
            <Metric
              icon={<ShieldCheck className="h-4 w-4" aria-hidden />}
              label="Snapshot"
              value={shortHash(latest.snapshotId)}
            />
            <Metric
              icon={<Database className="h-4 w-4" aria-hidden />}
              label="Assets"
              value={latest.liabilities.length.toString()}
            />
            <Metric
              icon={<RefreshCw className="h-4 w-4" aria-hidden />}
              label="Generated"
              value={formatTime(latest.generatedAt)}
            />
          </div>
          <div>
            <p className="mb-2 text-xs font-medium uppercase text-zinc-500">Merkle root</p>
            <code className="block rounded-md border border-zinc-800 bg-zinc-900 p-3 font-mono text-xs leading-5 text-emerald-200 break-all">
              {latest.merkleRoot}
            </code>
          </div>
          <Button type="button" onClick={onVerify} isLoading={isVerifying}>
            Verify my balance
          </Button>
        </div>
      </Panel>

      <Panel title="Total liabilities">
        <LiabilityTable latest={latest} />
      </Panel>
    </section>
  );
}

function MyProofView({ proof }: { proof: MyProof }) {
  return (
    <Panel title="My balance proof">
      {proof.entries.length === 0 ? (
        <p className="p-4 text-sm text-zinc-500">
          No positive balances are committed for this user.
        </p>
      ) : (
        <div className="grid gap-4 p-4 xl:grid-cols-[minmax(0,1fr)_360px]">
          <div className="flex flex-col gap-3">
            {proof.entries.map((entry) => (
              <ProofEntryCard key={`${entry.asset}-${entry.leafHash}`} entry={entry} />
            ))}
          </div>
          <div className="rounded-md border border-zinc-800 bg-zinc-900/50 p-3">
            <div className="mb-2 flex items-center gap-2 text-xs font-medium text-zinc-300">
              <KeyRound className="h-3.5 w-3.5 text-emerald-300" aria-hidden />
              Verifier snippet
            </div>
            <pre className="overflow-auto whitespace-pre-wrap break-words rounded bg-zinc-950 p-3 font-mono text-xs leading-5 text-zinc-400">
              {VERIFY_SNIPPET}
            </pre>
          </div>
        </div>
      )}
    </Panel>
  );
}

function ProofEntryCard({ entry }: { entry: ProofEntry }) {
  return (
    <article className="rounded-md border border-zinc-800 bg-zinc-900/50 p-3">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-medium text-zinc-100">{entry.asset}</h3>
          <p className="text-xs text-zinc-500">Committed balance {formatDecimal(entry.total)}</p>
        </div>
        <span className="rounded border border-emerald-900/70 bg-emerald-950/20 px-2 py-1 font-mono text-xs text-emerald-300">
          {shortHash(entry.leafHash)}
        </span>
      </div>
      <dl className="grid gap-2 text-xs">
        <ProofDatum label="Leaf payload" value={entry.leafPayload} />
        <ProofDatum label="Leaf hash" value={entry.leafHash} />
        <ProofDatum label="Siblings" value={entry.siblingHashes.join("\n") || "(single leaf)"} />
      </dl>
    </article>
  );
}

function ProofDatum({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="mb-1 text-zinc-500">{label}</dt>
      <dd className="whitespace-pre-wrap break-all font-mono text-zinc-300">{value}</dd>
    </div>
  );
}

function LiabilityTable({ latest }: { latest: ProofLatest }) {
  return (
    <div className="overflow-auto">
      <div className="grid min-w-[420px] grid-cols-[120px_1fr] border-b border-zinc-900 px-4 py-2 text-xs text-zinc-500">
        <span>Asset</span>
        <span className="text-right">Committed liability</span>
      </div>
      {latest.liabilities.map((liability) => (
        <div
          key={liability.asset}
          className="grid min-w-[420px] grid-cols-[120px_1fr] px-4 text-sm leading-10 hover:bg-zinc-900/70"
        >
          <span className="font-medium text-zinc-100">{liability.asset}</span>
          <span className="text-right text-zinc-300">{formatDecimal(liability.total)}</span>
        </div>
      ))}
    </div>
  );
}

function VerifyError({ error }: { error: Error }) {
  const message =
    error instanceof ApiError && error.status === 401
      ? "Sign in to generate your personal balance proof."
      : "Balance proof could not be generated.";
  return (
    <p className="rounded-md border border-amber-900/70 bg-amber-950/20 px-3 py-2 text-sm text-amber-200">
      {message}
    </p>
  );
}

function Metric({ icon, label, value }: { icon: ReactNode; label: string; value: string }) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-900/50 p-3">
      <div className="mb-2 flex h-7 w-7 items-center justify-center rounded bg-zinc-800 text-emerald-300">
        {icon}
      </div>
      <p className="text-xs text-zinc-500">{label}</p>
      <p className="mt-1 truncate text-sm font-medium text-zinc-100">{value}</p>
    </div>
  );
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="overflow-hidden rounded-md border border-zinc-800 bg-zinc-950">
      <div className="flex h-11 items-center border-b border-zinc-800 px-4">
        <h2 className="text-sm font-medium text-zinc-100">{title}</h2>
      </div>
      {typeof children === "string" ? (
        <p className="p-4 text-sm text-zinc-500">{children}</p>
      ) : (
        children
      )}
    </section>
  );
}

function shortHash(value: string): string {
  if (value.length <= ROOT_PREVIEW * 2) return value;
  return `${value.slice(0, ROOT_PREVIEW)}...${value.slice(-ROOT_PREVIEW)}`;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--";
  return new Intl.DateTimeFormat("en", {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    timeZone: "UTC",
  }).format(date);
}

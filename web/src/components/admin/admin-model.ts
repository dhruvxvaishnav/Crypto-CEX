import type { AdminEngineMarketState } from "@/types/api.types";

export type AdminStatusTone = "danger" | "muted" | "success";

export function statusTone(status: AdminEngineMarketState["status"]): AdminStatusTone {
  if (status === "trading") return "success";
  if (status === "halted") return "danger";
  return "muted";
}

export function totalOpenOrders(markets: AdminEngineMarketState[]): number {
  return markets.reduce((total, market) => total + market.openOrderCount, 0);
}

export function topOfBookText(level: readonly [string, string] | null): string {
  if (!level) return "--";
  return `${level[0]} x ${level[1]}`;
}

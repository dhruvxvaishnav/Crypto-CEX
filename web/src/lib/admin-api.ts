import { api } from "@/lib/api-client";
import type { AdminActionResponse, AdminEngineState } from "@/types/api.types";

export async function getAdminEngineState(): Promise<AdminEngineState> {
  return api.get<AdminEngineState>("/admin/engine/state");
}

export async function haltMarket(symbol: string): Promise<AdminActionResponse> {
  return api.post<AdminActionResponse>(`/admin/markets/${encodeURIComponent(symbol)}/halt`, {});
}

export async function resumeMarket(symbol: string): Promise<AdminActionResponse> {
  return api.post<AdminActionResponse>(`/admin/markets/${encodeURIComponent(symbol)}/resume`, {});
}

export async function cancelAllInMarket(symbol: string): Promise<AdminActionResponse> {
  return api.post<AdminActionResponse>(
    `/admin/markets/${encodeURIComponent(symbol)}/cancel-all`,
    {},
  );
}

export async function freezeUser(userId: string): Promise<AdminActionResponse> {
  return api.post<AdminActionResponse>(`/admin/users/${encodeURIComponent(userId)}/freeze`, {});
}

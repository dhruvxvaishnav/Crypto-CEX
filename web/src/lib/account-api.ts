import { api } from "@/lib/api-client";
import type {
  ApiDataResponse,
  ApiKey,
  ApiListResponse,
  Balance,
  CreateApiKeyInput,
  CreatedApiKey,
  FaucetInput,
  FaucetResponse,
  LedgerEntry,
  TotpSetupResponse,
  TotpVerifyResponse,
  UserProfile,
} from "@/types/api.types";

interface LedgerHistoryParams {
  cursor?: number | null;
  limit?: number;
}

interface DisableTotpInput {
  code: string;
  password: string;
}

const DEFAULT_LEDGER_LIMIT = 100;

export async function getProfile(): Promise<UserProfile> {
  return api.get<UserProfile>("/account");
}

export async function getBalances(): Promise<Balance[]> {
  const response = await api.get<ApiDataResponse<Balance[]>>("/account/balances");
  return response.data;
}

export async function getLedgerHistory(
  params: LedgerHistoryParams = {},
): Promise<ApiListResponse<LedgerEntry>> {
  const query = new URLSearchParams({
    limit: (params.limit ?? DEFAULT_LEDGER_LIMIT).toString(),
  });
  if (params.cursor !== undefined && params.cursor !== null) {
    query.set("cursor", params.cursor.toString());
  }
  return api.get<ApiListResponse<LedgerEntry>>(`/account/history?${query.toString()}`);
}

export async function requestFaucet(input: FaucetInput): Promise<FaucetResponse> {
  return api.post<FaucetResponse>("/wallet/faucet", input);
}

export async function setupTotp(): Promise<TotpSetupResponse> {
  return api.post<TotpSetupResponse>("/auth/2fa/setup", {});
}

export async function verifyTotp(code: string): Promise<TotpVerifyResponse> {
  return api.post<TotpVerifyResponse>("/auth/2fa/verify", { code });
}

export async function disableTotp(input: DisableTotpInput): Promise<void> {
  return api.post<void>("/auth/2fa/disable", input);
}

export async function listApiKeys(): Promise<ApiKey[]> {
  const response = await api.get<ApiDataResponse<ApiKey[]>>("/account/api-keys");
  return response.data;
}

export async function createApiKey(input: CreateApiKeyInput): Promise<CreatedApiKey> {
  return api.post<CreatedApiKey>("/account/api-keys", input);
}

export async function revokeApiKey(keyId: string): Promise<void> {
  await api.delete<void>(`/account/api-keys/${encodeURIComponent(keyId)}`);
}

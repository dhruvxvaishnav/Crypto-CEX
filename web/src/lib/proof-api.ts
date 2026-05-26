import { api } from "@/lib/api-client";
import type { MyProof, ProofLatest } from "@/types/api.types";

export async function getLatestProof(): Promise<ProofLatest> {
  return api.get<ProofLatest>("/proof-of-reserves/latest", { cache: "no-store" });
}

export async function getMyProof(): Promise<MyProof> {
  return api.get<MyProof>("/proof-of-reserves/me", { cache: "no-store" });
}

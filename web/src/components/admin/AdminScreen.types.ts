import type { AdminActionResponse, AdminEngineMarketState } from "@/types/api.types";

export interface ActionMessage {
  tone: "danger" | "success";
  text: string;
}

export interface MarketControlsProps {
  market: AdminEngineMarketState;
  onCancelAll: (symbol: string) => void;
  onHalt: (symbol: string) => void;
  onResume: (symbol: string) => void;
  pendingAction: string | null;
}

export interface EngineStateTableProps {
  markets: AdminEngineMarketState[];
}

export interface FreezeUserPanelProps {
  isPending: boolean;
  onFreeze: (userId: string) => void;
}

export interface AdminActionSummaryProps {
  response: AdminActionResponse | null;
}

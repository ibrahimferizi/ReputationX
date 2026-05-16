export type RiskLevel = "low" | "medium" | "high";

export interface WalletMetric {
  id: string;
  label: string;
  value: string;
  risk: RiskLevel;
  summary: string;
}

export interface ImprovementStep {
  metric_id: string;
  title: string;
  steps: string[];
}

export interface TokenRisk {
  mint: string;
  rugcheck_score: number | null;
  honeypot: boolean;
  flags: string[];
  available: boolean;
}

export interface ReputationReport {
  address: string;
  reputation_score: number;
  tier: string;
  celestila_tier: string;
  balance: { sol: number };
  tx_stats: { count: number; capped?: boolean; max_per_hour?: number };
  wallet_age: {
    days: number;
    days_precise?: number;
    first_activity_unix?: number | null;
  };
  nft_stats: { count: number };
  defi_exposure: { total_usd: number; interaction_count?: number };
  metrics: WalletMetric[];
  improvement_steps: ImprovementStep[];
  token_risks: TokenRisk[];
  integrations?: Record<string, unknown>;
  api_version?: string;
}

export interface HistoryEntry {
  address: string;
  tier: string;
  score: number;
  checkedAt: string;
}

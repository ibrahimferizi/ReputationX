export type RiskLevel = "low" | "medium" | "high" | "unknown";

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
  solsniffer_score?: number | null;
  solsniffer_checked_at?: string;
  honeypot: boolean;
  flags: string[];
  available: boolean;
}

export interface FundingSource {
  source_type: string;
  source_address: string | null;
  confidence: string;
}

export interface ReputationReport {
  address: string;
  generated_at?: string;
  reputation_score: number;
  tier: string;
  trust_label: string;
  funding_source?: FundingSource;
  balance: { sol: number };
  tx_stats: { count: number; capped?: boolean; sampled?: boolean; max_per_hour?: number };
  wallet_age: {
    days: number;
    days_precise?: number;
    first_activity_unix?: number | null;
    age_capped?: boolean;
  };
  nft_stats: { count: number };
  defi_exposure: { total_usd: number | null; interaction_count?: number | null };
  metrics: WalletMetric[];
  improvement_steps: ImprovementStep[];
  token_risks: TokenRisk[];
  integrations?: Record<string, unknown>;
  api_version?: string;
  scoring_version?: string;
  coverage?: ScanCoverage;
}

export interface HistoryEntry {
  address: string;
  tier: string;
  score: number;
  checkedAt: string;
}

export type ClusterSuspicion = "low" | "medium" | "high";

export interface WalletCluster {
  cluster_id: string;
  funding_address: string;
  members: string[];
  suspicion: ClusterSuspicion;
  reasons: string[];
}

export interface SybilScanResponse {
  wallets: ReputationReport[];
  clusters: WalletCluster[];
  total_scanned: number;
  flagged: number;
}

export type TokenScanStatus = "skipped" | "no_holdings" | "timed_out" | "unavailable" | "partial" | "complete";

export interface ScanCoverage {
  history_source: string;
  timed_transactions: number;
  sample_start_unix: number | null;
  sample_end_unix: number | null;
  token_scans: {
    status: TokenScanStatus;
    held_mints: number;
    selected_mints: number;
    checked_mints: number;
    fully_checked_mints: number;
    rugcheck_checked: number;
    solsniffer_checked: number;
    solsniffer_enabled: boolean;
    issues: Array<{ provider: string; mint: string; code: string; http_status: number | null; message: string }>;
  };
}

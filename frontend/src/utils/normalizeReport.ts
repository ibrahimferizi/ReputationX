import type { ReputationReport, RiskLevel, WalletMetric } from "../types/api";

function normalizeRisk(risk: unknown): RiskLevel {
  if (risk === "low" || risk === "medium" || risk === "high") {
    return risk;
  }
  return "medium";
}

/** Ensure API payload has arrays/objects the UI expects (avoids render crashes). */
export function normalizeReport(raw: Record<string, unknown>): ReputationReport {
  const metrics = Array.isArray(raw.metrics)
    ? (raw.metrics as WalletMetric[]).map((m) => ({
        id: String(m.id ?? ""),
        label: String(m.label ?? ""),
        value: String(m.value ?? ""),
        risk: normalizeRisk(m.risk),
        summary: String(m.summary ?? ""),
      }))
    : [];

  const improvement_steps = Array.isArray(raw.improvement_steps)
    ? raw.improvement_steps
    : [];

  const token_risks = Array.isArray(raw.token_risks) ? raw.token_risks : [];

  const balance =
    raw.balance && typeof raw.balance === "object"
      ? (raw.balance as { sol?: number })
      : { sol: 0 };

  const tx_stats =
    raw.tx_stats && typeof raw.tx_stats === "object"
      ? (raw.tx_stats as ReputationReport["tx_stats"])
      : { count: 0, capped: false };

  const wallet_age =
    raw.wallet_age && typeof raw.wallet_age === "object"
      ? (raw.wallet_age as ReputationReport["wallet_age"])
      : { days: 0 };

  const nft_stats =
    raw.nft_stats && typeof raw.nft_stats === "object"
      ? (raw.nft_stats as { count?: number })
      : { count: 0 };

  const defi_exposure =
    raw.defi_exposure && typeof raw.defi_exposure === "object"
      ? (raw.defi_exposure as ReputationReport["defi_exposure"])
      : { total_usd: 0 };

  return {
    address: String(raw.address ?? ""),
    reputation_score: Number(raw.reputation_score ?? 0),
    tier: String(raw.tier ?? "Mercury"),
    celestila_tier: String(raw.celestila_tier ?? raw.tier ?? "Mercury"),
    balance: { sol: Number(balance.sol ?? 0) },
    tx_stats: {
      count: Number(tx_stats.count ?? 0),
      capped: Boolean(tx_stats.capped),
      max_per_hour: tx_stats.max_per_hour,
    },
    wallet_age: {
      days: Number(wallet_age.days ?? 0),
      days_precise: wallet_age.days_precise,
      first_activity_unix: wallet_age.first_activity_unix ?? null,
    },
    nft_stats: { count: Number(nft_stats.count ?? 0) },
    defi_exposure: {
      total_usd: Number(defi_exposure.total_usd ?? 0),
      interaction_count: defi_exposure.interaction_count,
    },
    metrics,
    improvement_steps,
    token_risks,
    integrations:
      raw.integrations && typeof raw.integrations === "object"
        ? (raw.integrations as Record<string, unknown>)
        : undefined,
    api_version: raw.api_version ? String(raw.api_version) : undefined,
  };
}

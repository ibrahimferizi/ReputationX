import type { ReputationReport, RiskLevel, WalletMetric } from "../types/api";

function normalizeRisk(risk: unknown): RiskLevel {
  if (risk === "low" || risk === "medium" || risk === "high") {
    return risk;
  }
  return "medium";
}

function pick<T>(raw: Record<string, unknown>, snake: string, camel: string): T | undefined {
  if (raw[snake] !== undefined) return raw[snake] as T;
  if (raw[camel] !== undefined) return raw[camel] as T;
  return undefined;
}

function pickObject(
  raw: Record<string, unknown>,
  snake: string,
  camel: string
): Record<string, unknown> | undefined {
  const v = pick<Record<string, unknown>>(raw, snake, camel);
  return v && typeof v === "object" ? v : undefined;
}

/** Ensure API payload has arrays/objects the UI expects (avoids render crashes). */
export function normalizeReport(raw: Record<string, unknown>): ReputationReport {
  const metricsRaw = pick<unknown[]>(raw, "metrics", "metrics");
  const metrics = Array.isArray(metricsRaw)
    ? (metricsRaw as WalletMetric[]).map((m) => ({
        id: String(m.id ?? ""),
        label: String(m.label ?? ""),
        value: String(m.value ?? ""),
        risk: normalizeRisk(m.risk),
        summary: String(m.summary ?? ""),
      }))
    : [];

  const improvementRaw = pick<unknown[]>(raw, "improvement_steps", "improvementSteps");
  const improvement_steps = Array.isArray(improvementRaw) ? improvementRaw : [];

  const tokenRisksRaw = pick<unknown[]>(raw, "token_risks", "tokenRisks");
  const token_risks = Array.isArray(tokenRisksRaw) ? tokenRisksRaw : [];

  const balanceObj = pickObject(raw, "balance", "balance") ?? {};
  const txStatsObj = pickObject(raw, "tx_stats", "txStats") ?? {};
  const walletAgeObj = pickObject(raw, "wallet_age", "walletAge") ?? {};
  const nftStatsObj = pickObject(raw, "nft_stats", "nftStats") ?? {};
  const defiObj = pickObject(raw, "defi_exposure", "defiExposure") ?? {};

  const trustLabel = String(
    pick<string>(raw, "trust_label", "trustLabel") ??
      pick<string>(raw, "tier", "tier") ??
      "Unverified"
  );

  const reputationScore = Number(
    pick<number>(raw, "reputation_score", "reputationScore") ?? 0
  );

  return {
    address: String(raw.address ?? ""),
    reputation_score: reputationScore,
    tier: trustLabel,
    trust_label: trustLabel,
    balance: { sol: Number(balanceObj.sol ?? 0) },
    tx_stats: {
      count: Number(txStatsObj.count ?? 0),
      capped: Boolean(txStatsObj.capped),
      max_per_hour: txStatsObj.max_per_hour ?? txStatsObj.maxPerHour,
    },
    wallet_age: {
      days: Number(walletAgeObj.days ?? 0),
      days_precise:
        walletAgeObj.days_precise ?? walletAgeObj.daysPrecise,
      first_activity_unix:
        walletAgeObj.first_activity_unix ??
        walletAgeObj.firstActivityUnix ??
        null,
      age_capped: Boolean(
        walletAgeObj.age_capped ?? walletAgeObj.ageCapped ?? false
      ),
    },
    nft_stats: { count: Number(nftStatsObj.count ?? 0) },
    defi_exposure: {
      total_usd: Number(defiObj.total_usd ?? defiObj.totalUsd ?? 0),
      interaction_count:
        defiObj.interaction_count ?? defiObj.interactionCount,
    },
    metrics,
    improvement_steps,
    token_risks,
    integrations:
      raw.integrations && typeof raw.integrations === "object"
        ? (raw.integrations as Record<string, unknown>)
        : undefined,
    api_version: raw.api_version
      ? String(raw.api_version)
      : raw.apiVersion
        ? String(raw.apiVersion)
        : undefined,
  };
}

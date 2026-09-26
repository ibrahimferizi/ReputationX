import type { ReputationReport, RiskLevel, TokenScanStatus } from "../types/api";

function optionalNumber(value: unknown): number | undefined {
  if (value == null || value === "") return undefined;
  const number = Number(value);
  return Number.isFinite(number) ? number : undefined;
}

function objects(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value)
    ? value.filter((item): item is Record<string, unknown> =>
        item !== null && typeof item === "object" && !Array.isArray(item))
    : [];
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function normalizeRisk(risk: unknown): RiskLevel {
  if (risk === "low" || risk === "medium" || risk === "high") {
    return risk;
  }
  return "unknown";
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
  return v && typeof v === "object" && !Array.isArray(v) ? v : undefined;
}

/** Ensure API payload has arrays/objects the UI expects (avoids render crashes). */
export function normalizeReport(raw: Record<string, unknown>): ReputationReport {
  const metricsRaw = pick<unknown[]>(raw, "metrics", "metrics");
  const metrics = Array.isArray(metricsRaw)
    ? objects(metricsRaw).map((m) => ({
        id: String(m.id ?? ""),
        label: String(m.label ?? ""),
        value: String(m.value ?? ""),
        risk: normalizeRisk(m.risk),
        summary: String(m.summary ?? ""),
      }))
    : [];

  const improvementRaw = pick<unknown[]>(raw, "improvement_steps", "improvementSteps");
  const improvement_steps = objects(improvementRaw).map((step) => ({
    metric_id: String(step.metric_id ?? ""),
    title: String(step.title ?? ""),
    steps: strings(step.steps),
  }));

  const tokenRisksRaw = pick<unknown[]>(raw, "token_risks", "tokenRisks");
  const token_risks = objects(tokenRisksRaw).map((token) => ({
    mint: String(token.mint ?? ""),
    rugcheck_score: optionalNumber(token.rugcheck_score) ?? null,
    solsniffer_score: optionalNumber(token.solsniffer_score) ?? null,
    solsniffer_checked_at: typeof token.solsniffer_checked_at === "string" ? token.solsniffer_checked_at : undefined,
    honeypot: token.honeypot === true,
    flags: strings(token.flags),
    available: token.available === true,
  }));
  const funding = pickObject(raw, "funding_source", "fundingSource");

  const balanceObj = pickObject(raw, "balance", "balance") ?? {};
  const txStatsObj = pickObject(raw, "tx_stats", "txStats") ?? {};
  const walletAgeObj = pickObject(raw, "wallet_age", "walletAge") ?? {};
  const nftStatsObj = pickObject(raw, "nft_stats", "nftStats") ?? {};
  const defiObj = pickObject(raw, "defi_exposure", "defiExposure") ?? {};

  const coverage = pickObject(raw, "coverage", "coverage");
  const tokens = coverage ? pickObject(coverage, "token_scans", "tokenScans") : undefined;
  const statuses: TokenScanStatus[] = ["skipped", "no_holdings", "timed_out", "unavailable", "partial", "complete"];
  const tokenStatus = statuses.includes(tokens?.status as TokenScanStatus)
    ? tokens!.status as TokenScanStatus : "unavailable";

  const trustLabel = String(
    pick<string>(raw, "trust_label", "trustLabel") ??
      pick<string>(raw, "tier", "tier") ??
      "Activity unavailable"
  );

  const reputationScore = Number(
    pick<number>(raw, "reputation_score", "reputationScore") ?? 0
  );

  return {
    address: String(raw.address ?? ""),
    generated_at: typeof raw.generated_at === "string" ? raw.generated_at : undefined,
    reputation_score: reputationScore,
    tier: trustLabel,
    trust_label: trustLabel,
    funding_source: funding ? {
      source_type: String(funding.source_type ?? "unknown"),
      source_address: typeof funding.source_address === "string" ? funding.source_address : null,
      confidence: String(funding.confidence ?? "unknown"),
    } : undefined,
    balance: { sol: Number(balanceObj.sol ?? 0) },
    tx_stats: {
      count: Number(txStatsObj.count ?? 0),
      capped: Boolean(txStatsObj.capped),
      sampled: Boolean(txStatsObj.sampled ?? txStatsObj.capped),
      max_per_hour: optionalNumber(txStatsObj.max_per_hour ?? txStatsObj.maxPerHour),
    },
    wallet_age: {
      days: Number(walletAgeObj.days ?? 0),
      days_precise:
        optionalNumber(walletAgeObj.days_precise ?? walletAgeObj.daysPrecise),
      first_activity_unix:
        optionalNumber(walletAgeObj.first_activity_unix ??
        walletAgeObj.firstActivityUnix) ??
        null,
      age_capped: Boolean(
        walletAgeObj.age_capped ?? walletAgeObj.ageCapped ?? false
      ),
    },
    nft_stats: { count: Number(nftStatsObj.count ?? 0) },
    defi_exposure: {
      total_usd: optionalNumber(defiObj.total_usd ?? defiObj.totalUsd) ?? null,
      interaction_count:
        optionalNumber(defiObj.interaction_count ?? defiObj.interactionCount) ?? null,
    },
    metrics,
    improvement_steps,
    token_risks,
    integrations:
      raw.integrations && typeof raw.integrations === "object"
        ? (raw.integrations as Record<string, unknown>)
        : undefined,
    scoring_version: typeof raw.scoring_version === "string" ? raw.scoring_version : undefined,
    coverage: coverage && tokens ? {
      history_source: String(coverage.history_source ?? "unknown"),
      timed_transactions: optionalNumber(coverage.timed_transactions) ?? 0,
      sample_start_unix: optionalNumber(coverage.sample_start_unix) ?? null,
      sample_end_unix: optionalNumber(coverage.sample_end_unix) ?? null,
      token_scans: {
        status: tokenStatus,
        held_mints: optionalNumber(tokens.held_mints) ?? 0,
        selected_mints: optionalNumber(tokens.selected_mints) ?? 0,
        checked_mints: optionalNumber(tokens.checked_mints) ?? 0,
        fully_checked_mints: optionalNumber(tokens.fully_checked_mints) ?? 0,
        rugcheck_checked: optionalNumber(tokens.rugcheck_checked) ?? 0,
        solsniffer_checked: optionalNumber(tokens.solsniffer_checked) ?? 0,
        solsniffer_enabled: tokens.solsniffer_enabled === true,
        issues: objects(tokens.issues).map((issue) => ({
          provider: String(issue.provider ?? "unknown"), mint: String(issue.mint ?? ""),
          code: String(issue.code ?? "unknown"), http_status: optionalNumber(issue.http_status) ?? null,
          message: String(issue.message ?? "Provider result unavailable."),
        })),
      },
    } : undefined,
    api_version: raw.api_version
      ? String(raw.api_version)
      : raw.apiVersion
        ? String(raw.apiVersion)
        : undefined,
  };
}

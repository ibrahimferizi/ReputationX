import type { ReputationReport, TokenScanStatus } from "../types/api";

const TOKEN_STATUS: Record<TokenScanStatus, string> = {
  skipped: "Not checked", no_holdings: "No held SPL tokens", timed_out: "Scan timed out",
  unavailable: "Unavailable", partial: "Partial coverage", complete: "Checks completed",
};

export function coverageSummary(report: ReputationReport) {
  const token = report.coverage?.token_scans;
  const count = report.tx_stats.count;
  return {
    history: `${count} successful transaction${count === 1 ? "" : "s"} ${report.tx_stats.sampled ? "sampled" : "observed"}`,
    tokens: token ? TOKEN_STATUS[token.status] : "Coverage unavailable",
    tokenDetail: token ? `${token.checked_mints} of ${token.held_mints} detected held mints have a usable provider result; ${token.fully_checked_mints} checked by every enabled provider.` : "This report does not include scan coverage.",
  };
}

export function individualFlagReasons(wallet: ReputationReport): string[] {
  const reasons: string[] = [];
  if (wallet.token_risks.some((token) => token.honeypot)) reasons.push("Token provider reported a honeypot flag");
  wallet.metrics.filter((metric) => metric.risk === "high")
    .forEach((metric) => reasons.push(`${metric.label}: ${metric.summary}`));
  const medium = wallet.metrics.filter((metric) => metric.risk === "medium" &&
    ["wallet_age", "tx_burst", "tx_count", "tx_spacing", "token_risk"].includes(metric.id));
  if (medium.length >= 2) reasons.push(`Multiple moderate signals: ${medium.map((metric) => metric.label).join(", ")}`);
  return reasons;
}

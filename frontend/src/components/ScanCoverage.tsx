import type { ReputationReport } from "../types/api";
import { coverageSummary } from "../utils/coverage";

export function ScanCoverage({ report, compact = false }: { report: ReputationReport; compact?: boolean }) {
  const summary = coverageSummary(report);
  const coverage = report.coverage;
  const tokens = coverage?.token_scans;
  const period = coverage?.sample_start_unix != null && coverage.sample_end_unix != null
    ? `${new Date(coverage.sample_start_unix * 1000).toLocaleString()} – ${new Date(coverage.sample_end_unix * 1000).toLocaleString()}`
    : "Timestamps unavailable";

  return (
    <details className="scan-coverage" open={!compact}>
      <summary>Scan coverage · {summary.history} · Tokens: {summary.tokens}</summary>
      <dl>
        {report.generated_at && <><dt>Report generated</dt><dd>{new Date(report.generated_at).toLocaleString()}. Refresh may reuse a recent server report.</dd></>}
        <dt>Transaction history</dt>
        <dd>{summary.history}. This is the returned history, not a verified lifetime total.
          {coverage && <> Source: {coverage.history_source === "helius" ? "Helius" : "Solana RPC"}. {coverage.timed_transactions} transactions have timestamps.</>}
        </dd>
        <dt>Observed time window</dt><dd>{period}</dd>
        <dt>Wallet age</dt>
        <dd>{report.wallet_age.first_activity_unix == null ? "First activity unavailable." : report.wallet_age.age_capped ? "Lower bound only; older activity may exist." : "Based on first activity returned by the provider."}</dd>
        <dt>Token checks: {summary.tokens}</dt>
        <dd>{summary.tokenDetail}
          {tokens && <> RugCheck: {tokens.rugcheck_checked} usable results. SolSniffer: {tokens.solsniffer_enabled ? `${tokens.solsniffer_checked} usable results` : "not enabled"}.</>}
          {" "}Coverage is limited to detected legacy SPL holdings.
        </dd>
        {tokens && tokens.issues.length > 0 && <>
          <dt>Provider diagnostics</dt>
          <dd><ul>{tokens.issues.map((issue, index) => <li key={`${issue.provider}-${issue.mint}-${index}`}>
            {issue.provider === "rugcheck" ? "RugCheck" : "SolSniffer"} · <span className="mono" title={issue.mint}>{issue.mint.slice(0, 6)}…{issue.mint.slice(-4)}</span>
            {issue.http_status != null && <> · HTTP {issue.http_status}</>}: {issue.message}
          </li>)}</ul></dd>
        </>}
        <dt>Funding source</dt>
        <dd>{report.funding_source?.source_address
          ? <>{report.funding_source.source_type} · <span className="mono">{report.funding_source.source_address}</span> · {report.funding_source.confidence} confidence</>
          : "Unresolved; this wallet cannot be linked by shared funding in this scan."}</dd>
        <dt>DeFi activity</dt><dd>Not assessed. No interaction count or USD exposure is calculated.</dd>
      </dl>
    </details>
  );
}

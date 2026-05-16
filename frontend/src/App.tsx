import { useMemo, useState } from "react";
import { PublicKey } from "@solana/web3.js";
import { MetricRow } from "./components/MetricRow";
import { useWalletHistory } from "./hooks/useWalletHistory";
import type { ImprovementStep, ReputationReport } from "./types/api";
import { normalizeReport } from "./utils/normalizeReport";
import "./App.css";

const API_BASE = import.meta.env.VITE_API_URL || "http://localhost:3001";

function App() {
  const [walletAddress, setWalletAddress] = useState("");
  const [report, setReport] = useState<ReputationReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { history, addEntry, clearHistory } = useWalletHistory();

  const improvementByMetric = useMemo(() => {
    const map = new Map<string, ImprovementStep>();
    report?.improvement_steps?.forEach((step) => map.set(step.metric_id, step));
    return map;
  }, [report]);

  const checkRisk = async (addressOverride?: string) => {
    const address = (addressOverride ?? walletAddress).trim();
    setError(null);
    setLoading(true);

    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 45_000);

    try {
      new PublicKey(address);
      const response = await fetch(
        `${API_BASE}/api/reputation?address=${encodeURIComponent(address)}`,
        { signal: controller.signal }
      );

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(
          (errorData as { error?: string; details?: string }).error ||
            (errorData as { details?: string }).details ||
            "Failed to fetch reputation data"
        );
      }

      const raw = await response.json();
      const data = normalizeReport(raw as Record<string, unknown>);
      setReport(data);
      setWalletAddress(address);
      addEntry({
        address,
        tier: data.celestila_tier || data.tier,
        score: data.reputation_score,
      });
    } catch (err: unknown) {
      let message =
        err instanceof Error ? err.message : "Failed to check wallet";
      if (err instanceof DOMException && err.name === "AbortError") {
        message =
          "Request timed out after 45s. Set SCAN_MODE=fast and HELIUS_API_KEY in backend .env, then restart.";
      }
      setError(message);
    } finally {
      window.clearTimeout(timeoutId);
      setLoading(false);
    }
  };

  const tier = report?.celestila_tier || report?.tier || "";

  return (
    <div className="app">
      <header className="header">
        <h1>Wallet Guard</h1>
        <p>Solana wallet forensic preview — on-chain reputation API</p>
      </header>

      <div className="layout">
        <aside className="sidebar">
          <h3>Recent checks</h3>
          <p className="sidebar-hint">Stored in this browser session only (no login).</p>
          {history.length === 0 ? (
            <p className="muted">No wallets checked yet.</p>
          ) : (
            <ul className="history-list">
              {history.map((entry) => (
                <li key={entry.address}>
                  <button
                    type="button"
                    className="history-item"
                    onClick={() => checkRisk(entry.address)}
                  >
                    <span className="history-addr">{shorten(entry.address)}</span>
                    <span className="history-meta">
                      {entry.tier} · {entry.score}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          {history.length > 0 && (
            <button type="button" className="link-btn" onClick={clearHistory}>
              Clear history
            </button>
          )}
        </aside>

        <main className="main">
          <label htmlFor="wallet">Wallet address</label>
          <input
            id="wallet"
            type="text"
            value={walletAddress}
            onChange={(e) => setWalletAddress(e.target.value)}
            placeholder="Solana address"
            onKeyDown={(e) => {
              if (e.key === "Enter" && !loading && walletAddress.trim()) {
                checkRisk();
              }
            }}
          />

          <button
            type="button"
            className="primary-btn"
            onClick={() => checkRisk()}
            disabled={loading || !walletAddress.trim()}
          >
            {loading ? "Analyzing (usually 3–8s)…" : "Check wallet"}
          </button>

          {error && <div className="error-banner">{error}</div>}

          {report && (
            <section className="report">
              <div className="report-header">
                <div>
                  <h2>Reputation report</h2>
                  <p className="mono">{report.address}</p>
                </div>
                <div className="score-block">
                  <span className="score">{report.reputation_score}</span>
                  <span className="tier">{tier}</span>
                </div>
              </div>

              {report.tx_stats?.capped && (
                <p className="warn-banner">
                  Transaction history scan hit the server page limit. Use a dedicated RPC and
                  raise MAX_SIGNATURE_PAGES for full counts and exact wallet age on hyper-active
                  wallets.
                </p>
              )}

              <h3>Parameters</h3>
              <div className="metrics">
                {(report.metrics ?? []).map((metric) => (
                  <MetricRow
                    key={metric.id}
                    metric={metric}
                    improvement={improvementByMetric.get(metric.id)}
                  />
                ))}
              </div>

              {(report.token_risks?.length ?? 0) > 0 && (
                <>
                  <h3>Token scans (RugCheck / integrations)</h3>
                  <ul className="token-risks">
                    {(report.token_risks ?? []).map((t) => (
                      <li key={t.mint}>
                        <span className="mono">{shorten(t.mint)}</span>
                        {t.honeypot && <span className="tag danger">Honeypot</span>}
                        {t.rugcheck_score != null && (
                          <span className="tag">Score {t.rugcheck_score}</span>
                        )}
                      </li>
                    ))}
                  </ul>
                </>
              )}

              <p className="api-version">API {report.api_version || "walletguard-v1"}</p>
            </section>
          )}
        </main>
      </div>
    </div>
  );
}

function shorten(addr: string, chars = 6) {
  if (addr.length <= chars * 2 + 3) return addr;
  return `${addr.slice(0, chars)}…${addr.slice(-chars)}`;
}

export default App;

import { apiError } from "./utils/apiError";
import { useMemo, useState, type CSSProperties } from "react";
import { isSolanaAddress } from "./utils/address";
import { ScanCoverage } from "./components/ScanCoverage";
import { MetricRow } from "./components/MetricRow";
import { SybilScanner } from "./components/SybilScanner";
import { useWalletHistory } from "./hooks/useWalletHistory";
import type { ImprovementStep, ReputationReport } from "./types/api";
import { normalizeReport } from "./utils/normalizeReport";
import {
  getCachedWallet,
  setCachedWallet,
  formatCacheTimestamp,
  getCacheTimestamp,
} from "./utils/cache";
import "./App.css";

/** Empty = same-origin; Vite proxies /api → backend in dev. Set VITE_API_URL to override. */
const API_BASE = import.meta.env.VITE_API_URL || "";

type AppTab = "check" | "sybil";

function App() {
  const [activeTab, setActiveTab] = useState<AppTab>("check");
  const [walletAddress, setWalletAddress] = useState("");
  const [report, setReport] = useState<ReputationReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fromCache, setFromCache] = useState(false);
  const { history, addEntry, clearHistory } = useWalletHistory();

  const improvementByMetric = useMemo(() => {
    const map = new Map<string, ImprovementStep>();
    report?.improvement_steps?.forEach((step) => map.set(step.metric_id, step));
    return map;
  }, [report]);

  const checkRisk = async (addressOverride?: string, skipCache = false) => {
    const address = (addressOverride ?? walletAddress).trim();
    setError(null);
    setLoading(true);
    setFromCache(false);

    // Check cache first (unless force refresh)
    if (!skipCache) {
      const cached = getCachedWallet(address);
      if (cached) {
        setReport(cached);
        setWalletAddress(address);
        setFromCache(true);
        setLoading(false);
        addEntry({
          address,
          tier: cached.trust_label || cached.tier,
          score: cached.reputation_score,
        });
        return;
      }
    }

    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 180_000);

    try {
      if (!isSolanaAddress(address)) throw new Error("Enter a valid Solana address.");
      const response = await fetch(
        `${API_BASE}/api/reputation?address=${encodeURIComponent(address)}`,
        { signal: controller.signal }
      );

      if (!response.ok) throw await apiError(response);

      const raw = await response.json();
      const data = normalizeReport(raw as Record<string, unknown>);
      setReport(data);
      setWalletAddress(address);
      setFromCache(false);
      setCachedWallet(data);
      addEntry({
        address,
        tier: data.trust_label || data.tier,
        score: data.reputation_score,
      });
    } catch (err: unknown) {
      let message =
        err instanceof Error ? err.message : "Failed to check wallet";
      if (err instanceof DOMException && err.name === "AbortError") {
        message =
          "The request took too long. The free service may be waking up; wait a minute and retry.";
      }
      setError(message);
    } finally {
      window.clearTimeout(timeoutId);
      setLoading(false);
    }
  };

  const trustLabel = report?.trust_label || report?.tier || "";

  return (
    <div className="app">
      <header className="header">
        <h1>Wallet Guard</h1>
        <p>Solana wallet activity score (1–100) and scan evidence</p>
      </header>

      <nav
        style={{
          display: "flex",
          justifyContent: "center",
          gap: "0.5rem",
          marginBottom: "1.5rem",
        }}
        aria-label="Main"
      >
        <button
          type="button"
          onClick={() => setActiveTab("check")}
          style={tabButtonStyle(activeTab === "check")}
        >
          Wallet Check
        </button>
        <button
          type="button"
          onClick={() => setActiveTab("sybil")}
          style={tabButtonStyle(activeTab === "sybil")}
        >
          Sybil Scan
        </button>
      </nav>

      {activeTab === "sybil" ? (
        <SybilScanner />
      ) : (
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

          <div style={{ display: "flex", gap: "0.5rem" }}>
            <button
              type="button"
              className="primary-btn"
              onClick={() => checkRisk()}
              disabled={loading || !walletAddress.trim()}
            >
              {loading ? "Analyzing (usually 3–8s)…" : "Check wallet"}
            </button>
            <button
              type="button"
              className="secondary-btn"
              onClick={() => checkRisk(undefined, true)}
              disabled={loading || !walletAddress.trim()}
              style={{
                padding: "0.75rem 1.5rem",
                borderRadius: 8,
                border: "1px solid #333a45",
                background: "#1a1d24",
                color: "#fff",
                cursor: loading || !walletAddress.trim() ? "not-allowed" : "pointer",
                opacity: loading || !walletAddress.trim() ? 0.5 : 1,
              }}
            >
              {loading ? "Refreshing…" : "Refresh"}
            </button>
          </div>

          {error && <div className="error-banner">{error}</div>}

          {report && (
            <section className="report">
              {fromCache && (
                <div
                  style={{
                    margin: "0 0 1.25rem",
                    padding: "0.75rem 1rem",
                    background: "#1a2e1a",
                    borderRadius: 8,
                    border: "1px solid #2d4a2d",
                    fontWeight: 600,
                    color: "#a5d6a7",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                  }}
                >
                  <span>
                    Results loaded from cache ·{" "}
                    {getCacheTimestamp(report.address) && formatCacheTimestamp(getCacheTimestamp(report.address)!)}
                  </span>
                  <button
                    type="button"
                    onClick={() => checkRisk(undefined, true)}
                    style={{
                      padding: "0.4rem 0.8rem",
                      borderRadius: 4,
                      border: "1px solid #4a7c4a",
                      background: "#2d4a2d",
                      color: "#a5d6a7",
                      cursor: "pointer",
                      fontSize: "0.85rem",
                    }}
                  >
                    Update
                  </button>
                </div>
              )}
              <div className="report-header">
                <div>
                  <h2>Reputation report</h2>
                  <p className="mono">{report.address}</p>
                </div>
                <div className="score-block">
                  <span className="score">{report.reputation_score}</span>
                  <span className="score-suffix">/100</span>
                  <span className="tier">{trustLabel}</span>
                </div>
              </div>

              <p className="score-context">Heuristic score based on observed history, timing and SOL balance. Token findings are separate and do not change this score. It does not establish wallet safety or ownership.</p>
              <ScanCoverage report={report} />

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
                        <span className="mono" title={t.mint}>{shorten(t.mint)}</span>
                        {!t.available && <span className="tag">Result unavailable</span>}
                        {t.solsniffer_score != null && <span className="tag">SolSniffer score {t.solsniffer_score}/100</span>}
                        {t.flags.length > 0 && <span>{t.flags.join("; ")}</span>}
                        {t.solsniffer_checked_at && <span>SolSniffer checked {new Date(t.solsniffer_checked_at).toLocaleString()}</span>}
                        {t.honeypot && <span className="tag danger">Honeypot</span>}
                        {t.rugcheck_score != null && (
                          <span className="tag">RugCheck risk score {t.rugcheck_score} (higher = more risk)</span>
                        )}
                      </li>
                    ))}
                  </ul>
                </>
              )}

              <p className="api-version">API {report.api_version || "unknown"} · Scoring {report.scoring_version || "legacy"}</p>
            </section>
          )}
        </main>
      </div>
      )}
      <footer className="score-context" style={{maxWidth: 960, margin: "2rem auto", padding: "1rem"}}>
        <details>
          <summary>About this beta · Data and privacy</summary>
          <p>WalletGuard summarizes public Solana activity. Scores and shared-funding groups are unvalidated heuristics, not proof of identity, safety or common ownership. Missing data is disclosed in each report.</p>
          <p>Submitted addresses are sent to the API and its configured Solana/Helius providers; selected token mints are sent to RugCheck and, when enabled, SolSniffer. Reports are cached briefly in server memory and in this browser. Recent-check history lasts for this browser session. No wallet connection, signature or login is required.</p>
          <p>The API uses connection IP addresses for temporary rate limits. Hosting and API providers process requests under their own policies. The app includes no analytics. Free hosting can sleep, and scans may pause when usage limits are reached.</p>
          <button type="button" onClick={() => { for (const key of Object.keys(localStorage)) if (key.startsWith("walletguard_")) localStorage.removeItem(key); for (const key of Object.keys(sessionStorage)) if (key.startsWith("walletguard_")) sessionStorage.removeItem(key); window.location.reload(); }}>Clear WalletGuard data in this browser</button>
        </details>
      </footer>
    </div>
  );
}

function tabButtonStyle(active: boolean): CSSProperties {
  return {
    padding: "0.55rem 1.25rem",
    borderRadius: 8,
    border: active ? "1px solid #4a9eff" : "1px solid #333a45",
    background: active ? "#2563eb" : "#1a1d24",
    color: "#fff",
    fontWeight: 600,
    cursor: "pointer",
  };
}

function shorten(addr: string, chars = 6) {
  if (addr.length <= chars * 2 + 3) return addr;
  return `${addr.slice(0, chars)}…${addr.slice(-chars)}`;
}

export default App;

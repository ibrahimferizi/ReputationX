import { useMemo, useState } from "react";
import { ClusterGraph } from "./ClusterGraph";
import type { ClusterSuspicion, ReputationReport, SybilScanResponse } from "../types/api";
import {
  getCachedScan,
  setCachedScan,
  formatCacheTimestamp,
  getCacheTimestamp,
} from "../utils/cache";

/** Empty = same-origin; Vite proxies /api → backend in dev. Set VITE_API_URL to override. */
const API_BASE = import.meta.env.VITE_API_URL || "";
const MAX_ADDRESSES = 50;

const SUSPICION_STYLES: Record<
  ClusterSuspicion,
  { background: string; color: string; border: string }
> = {
  high: { background: "#7f1d1d", color: "#fecaca", border: "#b91c1c" },
  medium: { background: "#422006", color: "#fdba74", border: "#c2410c" },
  low: { background: "#422006", color: "#fde047", border: "#854d0e" },
};

export function SybilScanner() {
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SybilScanResponse | null>(null);
  const [fromCache, setFromCache] = useState(false);

  const runScan = async (skipCache = false) => {
    const addresses = input
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    if (addresses.length === 0) {
      setError("Paste at least one wallet address (one per line).");
      return;
    }
    if (addresses.length > MAX_ADDRESSES) {
      setError(`Maximum ${MAX_ADDRESSES} addresses per scan.`);
      return;
    }

    setError(null);
    setLoading(true);
    setResult(null);
    setFromCache(false);

    // Check cache first (unless force refresh)
    let addressesToFetch = addresses;
    if (!skipCache) {
      const { cached, cachedClusters, uncached } = getCachedScan(addresses);

      if (cached.size > 0 && uncached.length === 0) {
        // All wallets are cached
        const cachedWallets = Array.from(cached.values());
        const cachedResult: SybilScanResponse = {
          wallets: cachedWallets,
          clusters: cachedClusters,
          total_scanned: cachedWallets.length,
          flagged: cachedWallets.filter((w) => walletHasElevatedRisk(w)).length,
        };
        setResult(cachedResult);
        setFromCache(true);
        setLoading(false);
        return;
      }

      if (cached.size > 0) {
        // Partial cache hit - show cached results and fetch uncached only
        const cachedWallets = Array.from(cached.values());
        const partialResult: SybilScanResponse = {
          wallets: cachedWallets,
          clusters: cachedClusters,
          total_scanned: cachedWallets.length,
          flagged: cachedWallets.filter((w) => walletHasElevatedRisk(w)).length,
        };
        setResult(partialResult);
        setFromCache(true);
        // Continue to fetch uncached wallets only
        addressesToFetch = uncached;
      }
    }

    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 180_000);

    try {
      const response = await fetch(`${API_BASE}/api/sybil-scan`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ addresses: addressesToFetch }),
        signal: controller.signal,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(
          (errorData as { error?: string; details?: string }).error ||
            (errorData as { details?: string }).details ||
            "Sybil scan failed"
        );
      }

      const data = (await response.json()) as SybilScanResponse;
      
      // Merge with cached results if partial hit
      if (result && result.wallets.length > 0) {
        const mergedWallets = [...result.wallets, ...data.wallets];
        const mergedClusters = [...result.clusters, ...data.clusters];
        const mergedResult: SybilScanResponse = {
          wallets: mergedWallets,
          clusters: mergedClusters,
          total_scanned: mergedWallets.length,
          flagged: mergedWallets.filter((w) => walletHasElevatedRisk(w)).length,
        };
        setResult(mergedResult);
        setCachedScan(mergedResult);
      } else {
        setResult(data);
        setCachedScan(data);
      }
      setFromCache(false);
    } catch (err: unknown) {
      let message = err instanceof Error ? err.message : "Sybil scan failed";
      if (err instanceof DOMException && err.name === "AbortError") {
        message = "Scan timed out after 3 minutes. Try fewer addresses or use SCAN_MODE=fast.";
      } else if (err instanceof TypeError && message.toLowerCase().includes("fetch")) {
        message =
          "Could not reach the API. Start the backend (cd backend && cargo run) and restart the Vite dev server after pulling.";
      }
      setError(message);
    } finally {
      window.clearTimeout(timeoutId);
      setLoading(false);
    }
  };

  const flaggedSet = useMemo(() => {
    const set = new Set<string>();
    result?.clusters.forEach((cluster) => {
      cluster.members.forEach((addr) => set.add(addr));
    });
    result?.wallets.forEach((wallet) => {
      if (walletHasElevatedRisk(wallet)) {
        set.add(wallet.address);
      }
    });
    return set;
  }, [result]);

  const clusterFlaggedSet = useMemo(() => {
    const set = new Set<string>();
    result?.clusters.forEach((cluster) => {
      cluster.members.forEach((addr) => set.add(addr));
    });
    return set;
  }, [result]);

  const atRiskWallets = useMemo(() => {
    if (!result) return [];
    return result.wallets.filter((w) => flaggedSet.has(w.address));
  }, [result, flaggedSet]);

  const cleanWallets = useMemo(() => {
    if (!result) return [];
    return result.wallets
      .map((w) => w.address)
      .filter((addr) => !flaggedSet.has(addr));
  }, [result, flaggedSet]);

  const addressCount = input
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean).length;

  return (
    <div className="sybil-scanner" style={{ maxWidth: 960, margin: "0 auto" }}>
      <label htmlFor="sybil-addresses">Wallet addresses (one per line)</label>
      <textarea
        id="sybil-addresses"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="Paste Solana addresses…"
        rows={8}
        disabled={loading}
        style={{
          width: "100%",
          padding: "0.75rem 1rem",
          borderRadius: 8,
          border: "1px solid #333a45",
          background: "#1a1d24",
          color: "#fff",
          marginBottom: "0.5rem",
          fontFamily: "ui-monospace, monospace",
          fontSize: "0.85rem",
          resize: "vertical",
        }}
      />
      <p className="muted" style={{ margin: "0 0 1rem" }}>
        {addressCount} address{addressCount === 1 ? "" : "es"} · max {MAX_ADDRESSES}
      </p>

      <div style={{ display: "flex", gap: "0.5rem" }}>
        <button
          type="button"
          className="primary-btn"
          onClick={() => runScan(false)}
          disabled={loading || addressCount === 0}
        >
          {loading ? "Scanning wallets…" : "Scan"}
        </button>
        <button
          type="button"
          className="secondary-btn"
          onClick={() => runScan(true)}
          disabled={loading || addressCount === 0}
          style={{
            padding: "0.75rem 1.5rem",
            borderRadius: 8,
            border: "1px solid #333a45",
            background: "#1a1d24",
            color: "#fff",
            cursor: loading || addressCount === 0 ? "not-allowed" : "pointer",
            opacity: loading || addressCount === 0 ? 0.5 : 1,
          }}
        >
          {loading ? "Refreshing…" : "Force Refresh"}
        </button>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {loading && (
        <p className="muted" style={{ marginTop: "1.5rem", textAlign: "center" }}>
          Running reputation checks and cluster analysis…
        </p>
      )}

      {result && !loading && (
        <section className="report" style={{ marginTop: "2rem" }}>
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
              <span>Results loaded from cache</span>
              <button
                type="button"
                onClick={() => runScan(true)}
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
          <p
            style={{
              margin: "0 0 1.25rem",
              padding: "0.75rem 1rem",
              background: "#12151a",
              borderRadius: 8,
              border: "1px solid #2d333b",
              fontWeight: 600,
            }}
          >
            {result.total_scanned} wallet{result.total_scanned === 1 ? "" : "s"} scanned ·{" "}
            {result.flagged} flagged ({clusterFlaggedSet.size} in{" "}
            {result.clusters.length} cluster{result.clusters.length === 1 ? "" : "s"},{" "}
            {atRiskWallets.filter((w) => !clusterFlaggedSet.has(w.address)).length} elevated risk)
          </p>

          {atRiskWallets.length > 0 && (
            <>
              <h3 style={{ margin: "0 0 0.75rem" }}>Flagged wallets</h3>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem", marginBottom: "1.25rem" }}>
                {atRiskWallets.map((wallet) => (
                  <WalletResultRow
                    key={wallet.address}
                    wallet={wallet}
                    inCluster={clusterFlaggedSet.has(wallet.address)}
                  />
                ))}
              </div>
            </>
          )}

          {(result.clusters.length > 0 || cleanWallets.length > 0) && (
            <ClusterGraph 
              clusters={result.clusters} 
              cleanWallets={cleanWallets}
              elevatedRiskWallets={atRiskWallets.filter((w) => !clusterFlaggedSet.has(w.address))}
            />
          )}

          {result.clusters.length > 0 ? (
            <>
              <h3 style={{ margin: "0 0 0.75rem" }}>Clusters</h3>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                {result.clusters.map((cluster) => (
                  <ClusterCard key={cluster.cluster_id} cluster={cluster} />
                ))}
              </div>
            </>
          ) : (
            <p className="muted">No shared funding clusters detected.</p>
          )}

          <h3 style={{ margin: "1.5rem 0 0.75rem" }}>Clean wallets</h3>
          {cleanWallets.length === 0 ? (
            <p className="muted">No wallets passed without cluster or reputation flags.</p>
          ) : (
            <ul
              style={{
                listStyle: "none",
                margin: 0,
                padding: 0,
                display: "flex",
                flexDirection: "column",
                gap: "0.35rem",
              }}
            >
              {cleanWallets.map((addr) => (
                <li key={addr} className="mono" style={{ fontSize: "0.85rem" }}>
                  {addr}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

function walletHasElevatedRisk(wallet: ReputationReport): boolean {
  if (wallet.reputation_score <= 40) return true;
  if (wallet.token_risks?.some((t) => t.honeypot)) return true;
  if (wallet.metrics.some((m) => m.risk === "high")) return true;
  const mediumSignals = wallet.metrics.filter(
    (m) =>
      m.risk === "medium" &&
      ["wallet_age", "tx_burst", "tx_count", "tx_spacing", "token_risk"].includes(m.id),
  ).length;
  return mediumSignals >= 2;
}

function WalletResultRow({
  wallet,
  inCluster,
}: {
  wallet: ReputationReport;
  inCluster: boolean;
}) {
  const funder = wallet.funding_source;
  const highMetrics = wallet.metrics.filter((m) => m.risk === "high");
  const cacheTimestamp = getCacheTimestamp(wallet.address);

  return (
    <div
      className="metric-row"
      style={{
        padding: "0.65rem 0.85rem",
        textAlign: "left",
        fontSize: "0.85rem",
      }}
    >
      <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem", alignItems: "center" }}>
        <span className="mono" style={{ wordBreak: "break-all" }}>
          {wallet.address}
        </span>
        <span style={{ fontWeight: 600 }}>
          {wallet.trust_label} ({wallet.reputation_score}/100)
        </span>
        {inCluster && (
          <span style={{ fontSize: "0.72rem", color: "#fca5a5" }}>shared funder cluster</span>
        )}
        {!inCluster && <span style={{ fontSize: "0.72rem", color: "#fdba74" }}>elevated risk</span>}
        {cacheTimestamp && (
          <span style={{ fontSize: "0.72rem", color: "#639922" }}>
            cached · {formatCacheTimestamp(cacheTimestamp)}
          </span>
        )}
      </div>
      <p className="metric-row-summary" style={{ margin: "0.35rem 0 0" }}>
        Funder:{" "}
        {funder?.source_address ? (
          <span className="mono">
            {funder.source_type} · {funder.source_address}
          </span>
        ) : (
          <span>unknown (clustering needs a resolved funding source)</span>
        )}
      </p>
      {highMetrics.length > 0 && (
        <p style={{ margin: "0.25rem 0 0", color: "#8b949e", fontSize: "0.8rem" }}>
          High risk: {highMetrics.map((m) => m.label).join(", ")}
        </p>
      )}
    </div>
  );
}

function ClusterCard({ cluster }: { cluster: SybilScanResponse["clusters"][number] }) {
  const badge = SUSPICION_STYLES[cluster.suspicion] ?? SUSPICION_STYLES.low;

  return (
    <article
      className="metric-row"
      style={{ padding: "1rem" }}
    >
      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          alignItems: "center",
          gap: "0.5rem",
          marginBottom: "0.75rem",
        }}
      >
        <span className="mono" style={{ fontWeight: 600 }}>
          {cluster.cluster_id}
        </span>
        <span
          style={{
            fontSize: "0.75rem",
            padding: "0.2rem 0.55rem",
            borderRadius: 4,
            fontWeight: 600,
            textTransform: "uppercase",
            background: badge.background,
            color: badge.color,
            border: `1px solid ${badge.border}`,
          }}
        >
          {cluster.suspicion}
        </span>
      </div>
      <p className="metric-row-summary" style={{ margin: "0 0 0.5rem" }}>
        Funder: <span className="mono">{cluster.funding_address}</span>
      </p>
      <p style={{ margin: "0 0 0.35rem", fontSize: "0.85rem", color: "#c9d1d9" }}>
        Members ({cluster.members.length})
      </p>
      <ul style={{ margin: "0 0 0.75rem", paddingLeft: "1.2rem" }}>
        {cluster.members.map((member) => (
          <li key={member} className="mono" style={{ fontSize: "0.8rem", marginBottom: "0.2rem" }}>
            {member}
          </li>
        ))}
      </ul>
      <p style={{ margin: "0 0 0.35rem", fontSize: "0.85rem", color: "#c9d1d9" }}>
        Reasons
      </p>
      <ul style={{ margin: 0, paddingLeft: "1.2rem", fontSize: "0.85rem", color: "#8b949e" }}>
        {cluster.reasons.map((reason) => (
          <li key={reason}>{reason}</li>
        ))}
      </ul>
    </article>
  );
}

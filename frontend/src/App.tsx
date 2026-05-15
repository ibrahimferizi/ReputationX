import { useState } from "react";
import { Connection, PublicKey } from "@solana/web3.js";
import "./App.css";

const RPCEndpoint = import.meta.env.VITE_RPC_URL || "http://127.0.0.1:8899";
const connection = new Connection(RPCEndpoint);

function App() {
  const [walletAddress, setWalletAddress] = useState("");
  const [reputation, setReputation] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

    const checkRisk = async () => {
    setError(null);
    setReputation(null);
    setLoading(true);

    try {
      // Validate wallet address
      const pubKey = new PublicKey(walletAddress);
      console.log("Valid address:", pubKey.toBase58());

      // Call our backend proxy (which calls Identity Prism)
      const response = await fetch(
        `http://localhost:3001/api/reputation?address=${walletAddress}`
      );

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.error || "Failed to fetch reputation data");
      }

      const data = await response.json();
      setReputation(data);
    } catch (err: any) {
      setError(err.message || "Failed to check wallet risk");
    } finally {
      setLoading(false);
    }
  };

  const getRiskLevel = (tier: string) => {
    const lowRisk = ["Mercury", "Venus", "Earth"];
    const mediumRisk = ["Mars", "Jupiter"];
    const highRisk = ["Saturn", "Uranus", "Neptune", "Sun", "Binary Sun"];

    if (lowRisk.includes(tier)) return "Low Risk";
    if (mediumRisk.includes(tier)) return "Medium Risk";
    return "High Risk (Established wallet)";
  };

  const getRiskColor = (tier: string) => {
    const lowRisk = ["Mercury", "Venus", "Earth"];
    const mediumRisk = ["Mars", "Jupiter"];

    if (lowRisk.includes(tier)) return "#28a745"; // Green
    if (mediumRisk.includes(tier)) return "#ffc107"; // Yellow
    return "#dc3545"; // Red
  };

  return (
    <div
      style={{
        padding: "40px",
        fontFamily: "sans-serif",
        color: "#fff",
        background: "#111",
        minHeight: "100vh",
      }}
    >
      <h1 style={{ textAlign: "center" }}>Wallet Guard</h1>
      <p style={{ textAlign: "center", color: "#aaa" }}>
        Check any Solana wallet's reputation and risk level
      </p>

      <div style={{ maxWidth: "600px", margin: "0 auto", marginTop: "40px" }}>
        <label
          style={{
            display: "block",
            marginBottom: "10px",
            fontSize: "16px",
          }}
        >
          Enter Wallet Address:
        </label>
        <input
          type="text"
          value={walletAddress}
          onChange={(e) => setWalletAddress(e.target.value)}
          placeholder="e.g., vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg"
          style={{
            width: "100%",
            padding: "12px",
            fontSize: "14px",
            borderRadius: "6px",
            border: "1px solid #444",
            background: "#222",
            color: "#fff",
            marginBottom: "15px",
          }}
        />

        <button
          onClick={checkRisk}
          disabled={loading || !walletAddress}
          style={{
            width: "100%",
            padding: "14px",
            fontSize: "16px",
            cursor:"pointer",
            borderRadius: "6px",
            border: "none",
            background: loading ? "#666" : "#007bff",
            color: "#fff",
            fontWeight: "bold",
          }}
        >
          {loading ? "Checking..." : "Check Risk"}
        </button>

        {error && (
          <div
            style={{
              marginTop: "20px",
              padding: "15px",
              background: "#dc3545",
              color: "#fff",
              borderRadius: "6px",
            }}
          >
            <strong>Error:</strong> {error}
          </div>
        )}

        {reputation && (
          <div
            style={{
              marginTop: "30px",
              padding: "20px",
              background: "#222",
              borderRadius: "8px",
              border: "1px solid #444",
            }}
          >
            <h2 style={{ margin: "0 0 15px 0" }}>Reputation Report</h2>

            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                marginBottom: "15px",
              }}
            >
              <span style={{ fontSize: "16px" }}>
                <strong>Reputation Score:</strong> {reputation.reputation_score}
              </span>
              <span
                style={{
                  fontSize: "16px",
                  color: getRiskColor(reputation.celestila_tier || reputation.tier),
                  fontWeight: "bold",
                }}
              >
                Tier: {reputation.celestila_tier || reputation.tier}
              </span>
            </div>

            <div
              style={{
                padding: "15px",
                background: "#111",
                borderRadius: "6px",
                marginBottom: "15px",
              }}
            >
              <strong>Risk Level:</strong>{" "}
              <span style={{ color: getRiskColor(reputation.celestila_tier || reputation.tier) }}>
                {getRiskLevel(reputation.celestila_tier || reputation.tier)}
              </span>
            </div>

            <div style={{ fontSize: "14px", color: "#ccc" }}>
              <p><strong>Balance:</strong> {reputation.balance?.sol || 0} SOL</p>
              <p><strong>Transaction Count:</strong> {reputation.tx_stats?.count || 0}</p>
              <p><strong>Wallet Age:</strong> {reputation.wallet_age?.days || 0} days</p>
              <p><strong>NFT Count:</strong> {reputation.nft_stats?.count || 0}</p>
              <p><strong>DeFi Exposure:</strong> {reputation.defi_exposure?.total_usd || 0} USD</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
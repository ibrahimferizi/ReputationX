import { useState } from "react";
import type { ImprovementStep, WalletMetric } from "../types/api";
import { RiskIcon } from "./RiskIcon";

interface MetricRowProps {
  metric: WalletMetric;
  improvement?: ImprovementStep;
}

export function MetricRow({ metric, improvement }: MetricRowProps) {
  const [open, setOpen] = useState(false);
  const showImprove =
    improvement && (metric.risk === "high" || metric.risk === "medium");

  return (
    <div className="metric-row">
      <div className="metric-row-main">
        <RiskIcon risk={metric.risk} />
        <div className="metric-row-text">
          <div className="metric-row-label">
            <strong>{metric.label}</strong>
            <span className="metric-row-value">{metric.value}</span>
          </div>
          <p className="metric-row-summary">{metric.summary}</p>
        </div>
      </div>

      {showImprove && (
        <button
          type="button"
          className="improve-toggle"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? "Hide" : "Show"} steps to improve
        </button>
      )}

      {showImprove && open && (
        <div className="improve-panel">
          <strong>{improvement.title}</strong>
          <ul>
            {improvement.steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

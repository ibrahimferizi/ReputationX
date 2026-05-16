import type { RiskLevel } from "../types/api";

const ICONS: Record<RiskLevel, { glyph: string; color: string; label: string }> = {
  low: { glyph: "✓", color: "#28a745", label: "Low risk" },
  medium: { glyph: "!", color: "#ffc107", label: "Medium risk" },
  high: { glyph: "!", color: "#dc3545", label: "High risk" },
};

export function RiskIcon({ risk }: { risk: RiskLevel }) {
  const { glyph, color, label } = ICONS[risk] ?? ICONS.medium;
  return (
    <span
      title={label}
      aria-label={label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: 22,
        height: 22,
        borderRadius: "50%",
        background: color,
        color: risk === "medium" ? "#111" : "#fff",
        fontWeight: 700,
        fontSize: 14,
        flexShrink: 0,
      }}
    >
      {glyph}
    </span>
  );
}

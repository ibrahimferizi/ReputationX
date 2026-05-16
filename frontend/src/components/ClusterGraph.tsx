import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ClusterSuspicion, WalletCluster } from "../types/api";

const HEIGHT = 420;
const LINK_DIST = 80;
const REPULSION = 2800;
const SPRING = 0.03;
const GRAVITY = 0.002;
const DAMPING = 0.88;

const SUSPICION_COLORS: Record<ClusterSuspicion, string> = {
  high: "#E24B4A",
  medium: "#EF9F27",
  low: "#639922",
};

const FUNDER_RADIUS = 20;
const MEMBER_RADIUS = 11;
const CLEAN_RADIUS = 9;
const MEMBER_COLOR = "#7F77DD";
const CLEAN_COLOR = "#888780";

export interface ClusterGraphProps {
  clusters: WalletCluster[];
  cleanWallets: string[];
}

type NodeKind = "funder" | "member" | "clean";

interface SimNode {
  id: string;
  kind: NodeKind;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  opacity: number;
  label: string;
  address: string;
  cluster?: WalletCluster;
  pinned: boolean;
}

interface SimEdge {
  source: number;
  target: number;
  suspicion: ClusterSuspicion;
}

interface TooltipState {
  x: number;
  y: number;
  node: SimNode;
}

export function ClusterGraph({ clusters, cleanWallets }: ClusterGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const simRef = useRef<{ nodes: SimNode[]; edges: SimEdge[]; width: number } | null>(
    null,
  );
  const dragRef = useRef<{ index: number; offsetX: number; offsetY: number } | null>(null);
  const hoverRef = useRef<number | null>(null);
  const rafRef = useRef<number>(0);

  const [width, setWidth] = useState(700);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const [cursor, setCursor] = useState<"grab" | "pointer" | "grabbing">("grab");

  const graphKey = useMemo(
    () =>
      JSON.stringify({
        clusters: clusters.map((c) => ({
          id: c.cluster_id,
          funder: c.funding_address,
          members: c.members,
          suspicion: c.suspicion,
        })),
        clean: cleanWallets,
      }),
    [clusters, cleanWallets],
  );

  const buildSimulation = useCallback(
    (canvasWidth: number): { nodes: SimNode[]; edges: SimEdge[] } => {
      const nodes: SimNode[] = [];
      const edges: SimEdge[] = [];

      clusters.forEach((cluster, clusterIndex) => {
        const { x: fx, y: fy } = funderPosition(
          clusterIndex,
          clusters.length,
          canvasWidth,
          HEIGHT,
        );
        const suspicionColor = SUSPICION_COLORS[cluster.suspicion];
        const memberCount = cluster.members.length;
        const orbitRadius = 70 + memberCount * 5;

        const funderIndex = nodes.length;
        nodes.push({
          id: `funder-${cluster.cluster_id}`,
          kind: "funder",
          x: fx,
          y: fy,
          vx: 0,
          vy: 0,
          radius: FUNDER_RADIUS,
          color: suspicionColor,
          opacity: 1,
          label: truncateAddress(cluster.funding_address),
          address: cluster.funding_address,
          cluster,
          pinned: false,
        });

        cluster.members.forEach((member, memberIndex) => {
          const angle =
            memberCount > 0 ? (2 * Math.PI * memberIndex) / memberCount : 0;
          const mx = fx + orbitRadius * Math.cos(angle);
          const my = fy + orbitRadius * Math.sin(angle);
          const memberIndexInGraph = nodes.length;
          nodes.push({
            id: `member-${cluster.cluster_id}-${member}`,
            kind: "member",
            x: mx,
            y: my,
            vx: 0,
            vy: 0,
            radius: MEMBER_RADIUS,
            color: MEMBER_COLOR,
            opacity: 1,
            label: truncateAddress(member),
            address: member,
            cluster,
            pinned: false,
          });
          edges.push({
            source: funderIndex,
            target: memberIndexInGraph,
            suspicion: cluster.suspicion,
          });
        });
      });

      const cleanCount = cleanWallets.length;
      cleanWallets.forEach((address, index) => {
        const { x, y } = cleanWalletPosition(index, cleanCount, canvasWidth, HEIGHT);
        nodes.push({
          id: `clean-${address}`,
          kind: "clean",
          x,
          y,
          vx: 0,
          vy: 0,
          radius: CLEAN_RADIUS,
          color: CLEAN_COLOR,
          opacity: 0.7,
          label: truncateAddress(address),
          address,
          pinned: false,
        });
      });

      return { nodes, edges };
    },
    [clusters, cleanWallets],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const updateWidth = () => {
      setWidth(Math.max(320, container.clientWidth));
    };
    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const { nodes, edges } = buildSimulation(width);
    simRef.current = { nodes, edges, width };
    dragRef.current = null;
    hoverRef.current = null;
    setTooltip(null);
    setCursor("grab");
  }, [graphKey, width, buildSimulation]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;

    const tick = () => {
      const sim = simRef.current;
      if (!sim || sim.nodes.length === 0) {
        rafRef.current = requestAnimationFrame(tick);
        return;
      }

      const { nodes, edges } = sim;
      const w = sim.width;
      const h = HEIGHT;
      const cx = w / 2;
      const cy = h / 2;

      for (let i = 0; i < nodes.length; i++) {
        const a = nodes[i];
        if (a.pinned) continue;

        let fx = (cx - a.x) * GRAVITY;
        let fy = (cy - a.y) * GRAVITY;

        for (let j = i + 1; j < nodes.length; j++) {
          const b = nodes[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let distSq = dx * dx + dy * dy;
          if (distSq < 1) distSq = 1;
          const dist = Math.sqrt(distSq);
          const force = REPULSION / distSq;
          const fxPair = (force * dx) / dist;
          const fyPair = (force * dy) / dist;
          if (!a.pinned) {
            fx += fxPair;
            fy += fyPair;
          }
          if (!b.pinned) {
            b.vx -= fxPair;
            b.vy -= fyPair;
          }
        }

        a.vx += fx;
        a.vy += fy;
      }

      for (const edge of edges) {
        const a = nodes[edge.source];
        const b = nodes[edge.target];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 0.001;
        const displacement = dist - LINK_DIST;
        const force = SPRING * displacement;
        const fx = (force * dx) / dist;
        const fy = (force * dy) / dist;
        if (!a.pinned) {
          a.vx += fx;
          a.vy += fy;
        }
        if (!b.pinned) {
          b.vx -= fx;
          b.vy -= fy;
        }
      }

      for (const node of nodes) {
        if (node.pinned) continue;
        node.vx *= DAMPING;
        node.vy *= DAMPING;
        node.x += node.vx;
        node.y += node.vy;
        const pad = node.radius + 8;
        node.x = Math.max(pad, Math.min(w - pad, node.x));
        node.y = Math.max(pad, Math.min(h - pad - 14, node.y));
      }

      canvas.width = w * dpr;
      canvas.height = h * dpr;
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      for (const edge of edges) {
        const a = nodes[edge.source];
        const b = nodes[edge.target];
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.strokeStyle = withAlpha(SUSPICION_COLORS[edge.suspicion], 0.35);
        ctx.lineWidth = edge.suspicion === "high" ? 1.5 : 1;
        ctx.stroke();
      }

      for (const node of nodes) {
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
        ctx.fillStyle = withAlpha(node.color, node.opacity);
        ctx.fill();
      }

      for (const node of nodes) {
        ctx.fillStyle = `rgba(154, 160, 166, ${node.opacity})`;
        ctx.textAlign = "center";
        if (node.kind === "funder") {
          ctx.font = "500 10px ui-monospace, monospace";
        } else {
          ctx.font = "9px ui-monospace, monospace";
        }
        ctx.fillText(node.label, node.x, node.y + node.radius + 12);
      }

      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [graphKey, width]);

  const getCanvasPoint = (clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((clientX - rect.left) / rect.width) * width,
      y: ((clientY - rect.top) / rect.height) * HEIGHT,
    };
  };

  const findNodeAt = (x: number, y: number): number | null => {
    const sim = simRef.current;
    if (!sim) return null;
    for (let i = sim.nodes.length - 1; i >= 0; i--) {
      const node = sim.nodes[i];
      const dx = x - node.x;
      const dy = y - node.y;
      if (dx * dx + dy * dy <= node.radius * node.radius) return i;
    }
    return null;
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const point = getCanvasPoint(e.clientX, e.clientY);
    if (!point) return;
    const index = findNodeAt(point.x, point.y);
    if (index === null) return;
    const sim = simRef.current;
    if (!sim) return;
    const node = sim.nodes[index];
    node.pinned = true;
    dragRef.current = {
      index,
      offsetX: point.x - node.x,
      offsetY: point.y - node.y,
    };
    setCursor("grabbing");
    canvasRef.current?.setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const point = getCanvasPoint(e.clientX, e.clientY);
    if (!point) return;
    const sim = simRef.current;
    if (!sim) return;

    const drag = dragRef.current;
    if (drag) {
      const node = sim.nodes[drag.index];
      node.x = point.x - drag.offsetX;
      node.y = point.y - drag.offsetY;
      node.vx = 0;
      node.vy = 0;
      const pad = node.radius + 8;
      node.x = Math.max(pad, Math.min(sim.width - pad, node.x));
      node.y = Math.max(pad, Math.min(HEIGHT - pad - 14, node.y));
      setTooltip({
        x: e.clientX + 14,
        y: e.clientY + 14,
        node: { ...node },
      });
      return;
    }

    const index = findNodeAt(point.x, point.y);
    hoverRef.current = index;
    if (index !== null) {
      setCursor("pointer");
      setTooltip({
        x: e.clientX + 14,
        y: e.clientY + 14,
        node: { ...sim.nodes[index] },
      });
    } else {
      setCursor("grab");
      setTooltip(null);
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const sim = simRef.current;
    if (sim && dragRef.current) {
      sim.nodes[dragRef.current.index].pinned = false;
    }
    dragRef.current = null;
    canvasRef.current?.releasePointerCapture(e.pointerId);
    const point = getCanvasPoint(e.clientX, e.clientY);
    if (point) {
      const index = findNodeAt(point.x, point.y);
      setCursor(index !== null ? "pointer" : "grab");
      if (index !== null && sim) {
        setTooltip({
          x: e.clientX + 14,
          y: e.clientY + 14,
          node: { ...sim.nodes[index] },
        });
      } else {
        setTooltip(null);
      }
    } else {
      setCursor("grab");
      setTooltip(null);
    }
  };

  const handlePointerLeave = () => {
    if (!dragRef.current) {
      hoverRef.current = null;
      setTooltip(null);
      setCursor("grab");
    }
  };

  return (
    <div
      ref={containerRef}
      style={{ width: "100%", marginBottom: "1.25rem" }}
    >
      <canvas
        ref={canvasRef}
        width={width}
        height={HEIGHT}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerLeave}
        onPointerCancel={handlePointerUp}
        style={{
          display: "block",
          width: "100%",
          height: HEIGHT,
          cursor,
          background: "var(--color-background-secondary, #12151a)",
          borderRadius: "var(--border-radius-lg, 8px)",
          border: "0.5px solid var(--color-border-tertiary, #2d333b)",
        }}
        aria-label="Cluster network graph"
      />

      {tooltip && (
        <div
          role="tooltip"
          style={{
            position: "fixed",
            left: tooltip.x,
            top: tooltip.y,
            zIndex: 1000,
            maxWidth: 320,
            padding: "10px 12px",
            background: "var(--color-background-primary, #1a1d24)",
            border: "0.5px solid var(--color-border-tertiary, #2d333b)",
            borderRadius: "var(--border-radius-md, 6px)",
            boxShadow: "0 4px 12px rgba(0,0,0,0.35)",
            fontSize: 12,
            lineHeight: 1.45,
            textAlign: "left",
            pointerEvents: "none",
            color: "var(--color-text-primary, #e6edf3)",
          }}
        >
          <TooltipContent node={tooltip.node} />
        </div>
      )}

      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          alignItems: "center",
          gap: "12px 20px",
          marginTop: 12,
          fontSize: 11,
          color: "var(--color-text-secondary, #9aa0a6)",
        }}
      >
        {LEGEND_ITEMS.map((item) => (
          <span
            key={item.label}
            style={{ display: "inline-flex", alignItems: "center", gap: 6 }}
          >
            <span
              style={{
                width: item.dot * 2,
                height: item.dot * 2,
                borderRadius: "50%",
                background: item.color,
                opacity: item.opacity ?? 1,
                flexShrink: 0,
              }}
            />
            {item.label}
          </span>
        ))}
        <span style={{ marginLeft: "auto", fontStyle: "italic", opacity: 0.85 }}>
          Drag nodes to explore · Hover for details
        </span>
      </div>
    </div>
  );
}

function TooltipContent({ node }: { node: SimNode }) {
  if (node.kind === "clean") {
    return (
      <>
        <div style={{ fontWeight: 600, marginBottom: 4 }}>Clean wallet</div>
        <div style={{ fontFamily: "ui-monospace, monospace", wordBreak: "break-all" }}>
          {node.address}
        </div>
      </>
    );
  }

  const cluster = node.cluster;
  if (!cluster) return null;

  return (
    <>
      <div style={{ fontWeight: 600, marginBottom: 4 }}>
        {node.kind === "funder" ? "Cluster funder" : "Cluster member"}
      </div>
      <div
        style={{
          fontFamily: "ui-monospace, monospace",
          wordBreak: "break-all",
          marginBottom: 6,
        }}
      >
        {node.address}
      </div>
      <div>Suspicion: {cluster.suspicion}</div>
      <div style={{ marginTop: 4 }}>Cluster: {cluster.cluster_id}</div>
      <div>Members: {cluster.members.length}</div>
      {cluster.reasons.length > 0 && (
        <ul style={{ margin: "6px 0 0", paddingLeft: 18 }}>
          {cluster.reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      )}
    </>
  );
}

const LEGEND_ITEMS = [
  { color: SUSPICION_COLORS.high, dot: 8, label: "High suspicion funder" },
  { color: SUSPICION_COLORS.medium, dot: 8, label: "Medium suspicion funder" },
  { color: SUSPICION_COLORS.low, dot: 8, label: "Low suspicion funder" },
  { color: MEMBER_COLOR, dot: 7, label: "Cluster member wallet" },
  { color: CLEAN_COLOR, dot: 6, label: "Clean wallet", opacity: 0.7 },
] as const;

function funderPosition(
  index: number,
  total: number,
  width: number,
  height: number,
): { x: number; y: number } {
  const baseX = width * 0.28;
  const centerY = height / 2;

  if (total <= 1) {
    return { x: baseX, y: centerY };
  }

  const triangleAngles = [-Math.PI / 2, Math.PI / 6, (5 * Math.PI) / 6];
  const spread = Math.min(90, 55 + total * 4);
  const vertex = index % 3;
  const ring = Math.floor(index / 3);
  const angle = triangleAngles[vertex];
  const radius = spread + ring * 45;

  return {
    x: baseX + radius * Math.cos(angle),
    y: centerY + radius * Math.sin(angle),
  };
}

function cleanWalletPosition(
  index: number,
  total: number,
  width: number,
  height: number,
): { x: number; y: number } {
  const x = width * 0.82;
  const padding = 48;

  if (total <= 1) {
    return { x, y: height / 2 };
  }

  const usable = height - padding * 2;
  const y = padding + (usable * index) / (total - 1);
  return { x, y };
}

function truncateAddress(address: string): string {
  if (address.length <= 11) return address;
  return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

function withAlpha(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  const r = parseInt(normalized.slice(0, 2), 16);
  const g = parseInt(normalized.slice(2, 4), 16);
  const b = parseInt(normalized.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

import { useEffect, useRef } from "react";
import type { ClusterSuspicion, WalletCluster } from "../types/api";

const WIDTH = 700;
const HEIGHT = 420;
const LEFT_REGION_WIDTH = WIDTH * 0.6;
const ORBIT_RADIUS = 70;
const CENTER_NODE_RADIUS = 22;
const MEMBER_NODE_RADIUS = 12;
const CLEAN_NODE_RADIUS = 10;

const SUSPICION_COLORS: Record<ClusterSuspicion, string> = {
  high: "#ef4444",
  medium: "#f97316",
  low: "#eab308",
};

const MEMBER_COLOR = "#6366f1";
const CLEAN_COLOR = "#22c55e";
const LINE_COLOR = "#4b5563";
const BG_COLOR = "#12151a";
const LABEL_COLOR = "#9aa0a6";

export interface ClusterGraphProps {
  clusters: WalletCluster[];
  cleanWallets: string[];
}

interface NodeDraw {
  x: number;
  y: number;
  radius: number;
  color: string;
  label: string;
}

interface LineDraw {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export function ClusterGraph({ clusters, cleanWallets }: ClusterGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const lines: LineDraw[] = [];
    const nodes: NodeDraw[] = [];

    ctx.clearRect(0, 0, WIDTH, HEIGHT);
    ctx.fillStyle = BG_COLOR;
    ctx.fillRect(0, 0, WIDTH, HEIGHT);

    const clusterCount = clusters.length;
    clusters.forEach((cluster, clusterIndex) => {
      const { x: centerX, y: centerY } = clusterCenterPosition(clusterIndex, clusterCount);
      const centerColor = SUSPICION_COLORS[cluster.suspicion] ?? SUSPICION_COLORS.low;
      const memberCount = cluster.members.length;

      cluster.members.forEach((member, memberIndex) => {
        const angle =
          memberCount > 0 ? (2 * Math.PI * memberIndex) / memberCount : 0;
        const memberX = centerX + ORBIT_RADIUS * Math.cos(angle);
        const memberY = centerY + ORBIT_RADIUS * Math.sin(angle);

        lines.push({ x1: centerX, y1: centerY, x2: memberX, y2: memberY });
        nodes.push({
          x: memberX,
          y: memberY,
          radius: MEMBER_NODE_RADIUS,
          color: MEMBER_COLOR,
          label: truncateAddress(member),
        });
      });

      nodes.push({
        x: centerX,
        y: centerY,
        radius: CENTER_NODE_RADIUS,
        color: centerColor,
        label: truncateAddress(cluster.funding_address),
      });
    });

    const cleanCount = cleanWallets.length;
    cleanWallets.forEach((address, index) => {
      const { x, y } = cleanWalletPosition(index, cleanCount);
      nodes.push({
        x,
        y,
        radius: CLEAN_NODE_RADIUS,
        color: CLEAN_COLOR,
        label: truncateAddress(address),
      });
    });

    for (const line of lines) {
      ctx.beginPath();
      ctx.moveTo(line.x1, line.y1);
      ctx.lineTo(line.x2, line.y2);
      ctx.strokeStyle = LINE_COLOR;
      ctx.lineWidth = 1;
      ctx.stroke();
    }

    for (const node of nodes) {
      ctx.beginPath();
      ctx.arc(node.x, node.y, node.radius, 0, 2 * Math.PI);
      ctx.fillStyle = node.color;
      ctx.fill();
      ctx.strokeStyle = "#1a1d24";
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }

    ctx.fillStyle = LABEL_COLOR;
    ctx.font = "9px ui-monospace, monospace";
    ctx.textAlign = "center";
    for (const node of nodes) {
      ctx.fillText(node.label, node.x, node.y + node.radius + 11);
    }

    drawLegend(ctx);
  }, [clusters, cleanWallets]);

  return (
    <canvas
      ref={canvasRef}
      width={WIDTH}
      height={HEIGHT}
      style={{
        display: "block",
        maxWidth: "100%",
        margin: "0 0 1.25rem",
        borderRadius: 8,
        border: "1px solid #2d333b",
      }}
      aria-label="Cluster network graph"
    />
  );
}

function clusterCenterPosition(index: number, total: number): { x: number; y: number } {
  const x = LEFT_REGION_WIDTH * 0.42;
  const paddingY = 56;
  const usableHeight = HEIGHT - paddingY * 2;

  if (total <= 1) {
    return { x, y: HEIGHT / 2 };
  }

  const y = paddingY + (usableHeight * index) / (total - 1);
  return { x, y };
}

function cleanWalletPosition(index: number, total: number): { x: number; y: number } {
  const baseX = WIDTH * 0.78;
  const baseY = HEIGHT / 2;
  const spreadRadius = Math.min(90, 28 + total * 6);

  if (total <= 1) {
    return { x: baseX, y: baseY };
  }

  const angle = (2 * Math.PI * index) / total - Math.PI / 2;
  return {
    x: baseX + spreadRadius * Math.cos(angle) * 0.85,
    y: baseY + spreadRadius * Math.sin(angle) * 0.65,
  };
}

function truncateAddress(address: string): string {
  if (address.length <= 11) return address;
  return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

function drawLegend(ctx: CanvasRenderingContext2D) {
  const items: { color: string; radius: number; text: string }[] = [
    { color: SUSPICION_COLORS.high, radius: 7, text: "High risk cluster center" },
    { color: SUSPICION_COLORS.medium, radius: 7, text: "Medium risk cluster center" },
    { color: SUSPICION_COLORS.low, radius: 7, text: "Low risk cluster center" },
    { color: MEMBER_COLOR, radius: 6, text: "Cluster member wallet" },
    { color: CLEAN_COLOR, radius: 5, text: "Clean wallet" },
  ];

  const startX = 12;
  let y = HEIGHT - 12 - items.length * 16;

  ctx.font = "10px system-ui, sans-serif";
  ctx.textAlign = "left";
  ctx.fillStyle = "#c9d1d9";

  for (const item of items) {
    ctx.beginPath();
    ctx.arc(startX + item.radius, y + item.radius, item.radius, 0, 2 * Math.PI);
    ctx.fillStyle = item.color;
    ctx.fill();
    ctx.fillStyle = "#c9d1d9";
    ctx.fillText(item.text, startX + item.radius * 2 + 8, y + item.radius + 3);
    y += 16;
  }
}

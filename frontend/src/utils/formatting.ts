export function formatBps(bps: number): string {
  if (bps >= 1_000_000_000) return `${(bps / 1_000_000_000).toFixed(1)} Gbps`;
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`;
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(1)} Kbps`;
  return `${bps} bps`;
}

export function formatUtilization(util: number): string {
  return `${(util * 100).toFixed(1)}%`;
}

export function utilizationColor(util: number): string {
  if (util <= 0.01) return "#4a5568"; // idle gray
  if (util < 0.3) return "#48bb78"; // green
  if (util < 0.6) return "#ecc94b"; // yellow
  if (util < 0.8) return "#ed8936"; // orange
  return "#f56565"; // red
}

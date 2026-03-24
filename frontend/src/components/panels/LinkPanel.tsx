import { useSelectionStore } from "../../store/selectionStore.ts";
import { useTopologyStore } from "../../store/topologyStore.ts";
import { formatBps, formatUtilization } from "../../utils/formatting.ts";
import * as api from "../../api/client.ts";

export default function LinkPanel() {
  const { selectedNodeId, selectedNodeType } = useSelectionStore();
  const { links, linkUtilization, autonomousSystems } = useTopologyStore();

  if (selectedNodeType !== "link" || !selectedNodeId) return null;

  const link = links[selectedNodeId];
  if (!link) return <div style={styles.empty}>Link not found</div>;

  const util = linkUtilization[selectedNodeId] ?? 0;

  // Find the routers
  const allRouters = Object.values(autonomousSystems).flatMap((as_) =>
    Object.values(as_.routers),
  );
  const routerA = allRouters.find((r) =>
    Object.keys(r.interfaces).includes(link.interface_a_id),
  );
  const routerB = allRouters.find((r) =>
    Object.keys(r.interfaces).includes(link.interface_b_id),
  );

  const handleToggle = async () => {
    await api.setLinkState(selectedNodeId!, !link.is_up);
  };

  return (
    <div style={styles.panel}>
      <h3 style={styles.title}>Link</h3>
      <div style={styles.meta}>
        <div>
          {routerA?.name ?? "?"} --- {routerB?.name ?? "?"}
        </div>
        <div>Bandwidth: {formatBps(link.bandwidth)}</div>
        <div>Delay: {link.delay_ms}ms</div>
        <div>Load: {formatBps(link.current_load)}</div>
        <div>Utilization: {formatUtilization(util)}</div>
        <div>
          Status:{" "}
          <span style={{ color: link.is_up ? "#48bb78" : "#f56565" }}>
            {link.is_up ? "UP" : "DOWN"}
          </span>
        </div>
      </div>

      <button style={styles.btn} onClick={handleToggle}>
        {link.is_up ? "Fail Link" : "Restore Link"}
      </button>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  panel: { padding: 16 },
  empty: { padding: 24, color: "#8b949e", textAlign: "center" },
  title: { margin: 0, fontSize: 16, color: "#e2e8f0" },
  meta: { marginTop: 8, fontSize: 12, color: "#8b949e", lineHeight: 1.8 },
  btn: {
    marginTop: 16,
    padding: "6px 14px",
    background: "#21262d",
    color: "#c9d1d9",
    border: "1px solid #30363d",
    borderRadius: 6,
    cursor: "pointer",
    fontSize: 13,
  },
};

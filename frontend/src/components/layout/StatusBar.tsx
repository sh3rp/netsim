import { useSimulationStore } from "../../store/simulationStore.ts";
import { useTopologyStore, getAllRouters } from "../../store/topologyStore.ts";

export default function StatusBar() {
  const { tick, running } = useSimulationStore();
  const { autonomousSystems, links } = useTopologyStore();

  const routerCount = getAllRouters(autonomousSystems).length;
  const asCount = Object.keys(autonomousSystems).length;
  const linkCount = Object.keys(links).length;

  return (
    <div style={styles.bar}>
      <span style={styles.item}>
        Status: {running ? "Running" : "Stopped"}
      </span>
      <span style={styles.item}>Tick: {tick}</span>
      <span style={styles.item}>ASes: {asCount}</span>
      <span style={styles.item}>Routers: {routerCount}</span>
      <span style={styles.item}>Links: {linkCount}</span>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  bar: {
    display: "flex",
    gap: 24,
    padding: "4px 16px",
    background: "#161b22",
    borderTop: "1px solid #30363d",
    fontSize: 12,
    color: "#8b949e",
    flexShrink: 0,
  },
  item: {
    fontFamily: "monospace",
  },
};

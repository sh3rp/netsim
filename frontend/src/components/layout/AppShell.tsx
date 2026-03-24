import { useEffect } from "react";
import Toolbar from "./Toolbar.tsx";
import StatusBar from "./StatusBar.tsx";
import TopologyCanvas from "../topology/TopologyCanvas.tsx";
import RouterPanel from "../panels/RouterPanel.tsx";
import LinkPanel from "../panels/LinkPanel.tsx";
import { useSelectionStore } from "../../store/selectionStore.ts";
import { useTopologyStore } from "../../store/topologyStore.ts";
import { useSimulationStore } from "../../store/simulationStore.ts";
import * as ws from "../../api/websocket.ts";
import * as api from "../../api/client.ts";

export default function AppShell() {
  const { selectedNodeType } = useSelectionStore();
  const { setTopology, updateLinkUtilization } = useTopologyStore();
  const { setTick, setFlows } = useSimulationStore();

  // Connect WebSocket and poll for updates
  useEffect(() => {
    ws.connect();

    const unsub = ws.onTick((update) => {
      setTick(update.tick);
      updateLinkUtilization(update.changes.link_utilization);
      setFlows(update.changes.traffic_flows);
    });

    // Initial topology load
    api.getTopologySnapshot().then(setTopology).catch(console.error);
    api.getSimState().then((s) => {
      setTick(s.tick);
      useSimulationStore.getState().setRunning(s.running);
      useSimulationStore.getState().setTickRate(s.tick_rate_ms);
    }).catch(console.error);

    return () => {
      unsub();
      ws.disconnect();
    };
  }, []);

  // Periodically refresh topology when simulation is running
  useEffect(() => {
    const interval = setInterval(() => {
      if (useSimulationStore.getState().running) {
        api.getTopologySnapshot().then(setTopology).catch(() => {});
      }
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div style={styles.shell}>
      <Toolbar />
      <div style={styles.main}>
        <div style={styles.canvas}>
          <TopologyCanvas />
        </div>
        <div style={styles.detailPanel}>
          {selectedNodeType === "link" ? <LinkPanel /> : <RouterPanel />}
        </div>
      </div>
      <StatusBar />
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  shell: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    width: "100vw",
    overflow: "hidden",
  },
  main: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  canvas: {
    flex: 7,
    position: "relative",
    minWidth: 0,
  },
  detailPanel: {
    flex: 3,
    background: "#161b22",
    borderLeft: "1px solid #30363d",
    overflowY: "auto",
    minWidth: 300,
    maxWidth: 450,
  },
};

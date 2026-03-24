import { useSimulationStore } from "../../store/simulationStore.ts";
import * as api from "../../api/client.ts";
import { useTopologyStore } from "../../store/topologyStore.ts";

export default function Toolbar() {
  const { tick, running, tickRateMs, setRunning, setTickRate, setTick } =
    useSimulationStore();
  const { setTopology } = useTopologyStore();

  const handleStart = async () => {
    await api.startSim();
    setRunning(true);
  };

  const handleStop = async () => {
    await api.stopSim();
    setRunning(false);
  };

  const handleStep = async () => {
    const result = await api.stepSim();
    setTick(result.tick);
  };

  const handleTickRateChange = async (
    e: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const ms = parseInt(e.target.value, 10);
    setTickRate(ms);
    await api.setTickRate(ms);
  };

  const handleLoadSample = async () => {
    await api.loadSample();
    const snapshot = await api.getTopologySnapshot();
    setTopology(snapshot);
    setTick(0);
    setRunning(false);
  };

  const handleSave = async () => {
    const json = await api.saveSim();
    const blob = new Blob([JSON.stringify(json, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "netsim-topology.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div style={styles.toolbar}>
      <div style={styles.brand}>NetSim</div>

      <div style={styles.controls}>
        <button
          style={styles.btn}
          onClick={running ? handleStop : handleStart}
        >
          {running ? "Pause" : "Play"}
        </button>
        <button style={styles.btn} onClick={handleStep} disabled={running}>
          Step
        </button>
        <span style={styles.tick}>Tick: {tick}</span>
      </div>

      <div style={styles.controls}>
        <label style={styles.label}>
          Speed:
          <input
            type="range"
            min="50"
            max="2000"
            step="50"
            value={tickRateMs}
            onChange={handleTickRateChange}
            style={{ marginLeft: 8, width: 100 }}
          />
          <span style={styles.label}>{tickRateMs}ms</span>
        </label>
      </div>

      <div style={styles.controls}>
        <button style={styles.btn} onClick={handleLoadSample}>
          Load Sample
        </button>
        <button style={styles.btn} onClick={handleSave}>
          Save
        </button>
        <button style={styles.btn} onClick={() => api.exportGns3()}>
          Export GNS3
        </button>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  toolbar: {
    display: "flex",
    alignItems: "center",
    gap: 24,
    padding: "8px 16px",
    background: "#161b22",
    borderBottom: "1px solid #30363d",
    flexShrink: 0,
  },
  brand: {
    fontSize: 18,
    fontWeight: "bold",
    color: "#58a6ff",
    marginRight: 16,
  },
  controls: {
    display: "flex",
    alignItems: "center",
    gap: 8,
  },
  btn: {
    padding: "6px 14px",
    background: "#21262d",
    color: "#c9d1d9",
    border: "1px solid #30363d",
    borderRadius: 6,
    cursor: "pointer",
    fontSize: 13,
  },
  tick: {
    color: "#8b949e",
    fontSize: 13,
    fontFamily: "monospace",
    marginLeft: 8,
  },
  label: {
    color: "#8b949e",
    fontSize: 12,
  },
};

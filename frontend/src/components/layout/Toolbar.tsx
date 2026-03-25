import { useState, useRef, useEffect, useCallback } from "react";
import { useSimulationStore } from "../../store/simulationStore.ts";
import { useTopologyStore } from "../../store/topologyStore.ts";
import * as api from "../../api/client.ts";

interface MenuItem {
  label: string;
  action: () => void;
  disabled?: boolean;
  separator?: false;
}

interface MenuSeparator {
  separator: true;
}

type MenuEntry = MenuItem | MenuSeparator;

interface MenuDef {
  label: string;
  items: MenuEntry[];
}

export default function Toolbar() {
  const { tick, running, tickRateMs, setRunning, setTickRate, setTick } =
    useSimulationStore();
  const { setTopology } = useTopologyStore();
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const menuBarRef = useRef<HTMLDivElement>(null);

  const closeMenu = useCallback(() => setOpenMenu(null), []);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (
        menuBarRef.current &&
        !menuBarRef.current.contains(e.target as Node)
      ) {
        closeMenu();
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [closeMenu]);

  const handleNew = async () => {
    if (!confirm("Create a new empty network? Unsaved changes will be lost."))
      return;
    await api.resetSim();
    const snapshot = await api.getTopologySnapshot();
    setTopology(snapshot);
    setTick(0);
    setRunning(false);
  };

  const handleOpen = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const text = await file.text();
      await api.loadSim(text);
      const snapshot = await api.getTopologySnapshot();
      setTopology(snapshot);
      setTick(0);
      setRunning(false);
    };
    input.click();
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

  const handleImport = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".gns3";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      // GNS3 import would need a backend endpoint; for now trigger the sample load
      // as a placeholder if no import endpoint exists
      await api.loadSample();
      const snapshot = await api.getTopologySnapshot();
      setTopology(snapshot);
      setTick(0);
      setRunning(false);
    };
    input.click();
  };

  const handleNewASN = async () => {
    const asnStr = window.prompt("Enter ASN number:");
    if (!asnStr) return;
    const asn = parseInt(asnStr, 10);
    if (isNaN(asn) || asn <= 0) {
      alert("Invalid ASN number.");
      return;
    }
    const name = window.prompt("Enter AS name:", `AS${asn}`) || `AS${asn}`;
    try {
      await api.createAS(asn, name);
      const snapshot = await api.getTopologySnapshot();
      setTopology(snapshot);
    } catch (e: any) {
      alert("Failed to create AS: " + e.message);
    }
  };

  const handleNewNode = async () => {
    // Read fresh state to avoid stale closures
    const currentAS = useTopologyStore.getState().autonomousSystems;
    const asList = Object.values(currentAS);
    if (asList.length === 0) {
      alert("No autonomous systems exist. Create an ASN first.");
      return;
    }

    let selectedAs;
    if (asList.length === 1) {
      selectedAs = asList[0];
    } else {
      const options = asList
        .map((a) => `  AS${a.asn} - ${a.name}`)
        .join("\n");
      const choice = window.prompt(
        `Enter the ASN number:\n${options}\n\nASN:`,
      );
      if (!choice) return;
      const asn = parseInt(choice, 10);
      selectedAs = asList.find((a) => a.asn === asn);
      if (!selectedAs) {
        alert(`ASN ${asn} not found.`);
        return;
      }
    }

    const name = window.prompt("Enter router name:");
    if (!name) return;
    const routerIp = window.prompt("Enter router ID IP (e.g. 10.0.0.1):");
    if (!routerIp) return;
    try {
      await api.createRouter(selectedAs.id, name, routerIp, 200, 200);
      const snapshot = await api.getTopologySnapshot();
      setTopology(snapshot);
    } catch (e: any) {
      alert("Failed to create router: " + e.message);
    }
  };

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

  const menus: MenuDef[] = [
    {
      label: "File",
      items: [
        { label: "New", action: handleNew },
        { label: "Open...", action: handleOpen },
        { label: "Save", action: handleSave },
        { separator: true },
        { label: "Import GNS3...", action: handleImport },
      ],
    },
    {
      label: "Topology",
      items: [
        { label: "New ASN", action: handleNewASN },
        { label: "New Node", action: handleNewNode },
      ],
    },
    {
      label: "Run",
      items: [
        {
          label: running ? "Stop" : "Start",
          action: running ? handleStop : handleStart,
        },
        { label: "Step", action: handleStep, disabled: running },
      ],
    },
  ];

  const handleTickRateChange = async (
    e: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const ms = parseInt(e.target.value, 10);
    setTickRate(ms);
    await api.setTickRate(ms);
  };

  return (
    <div style={styles.toolbar}>
      <div style={styles.brand}>NetSim</div>

      <div ref={menuBarRef} style={styles.menuBar}>
        {menus.map((menu) => (
          <div key={menu.label} style={styles.menuContainer}>
            <button
              style={{
                ...styles.menuButton,
                ...(openMenu === menu.label ? styles.menuButtonActive : {}),
              }}
              onClick={() =>
                setOpenMenu(openMenu === menu.label ? null : menu.label)
              }
              onMouseEnter={() => {
                if (openMenu !== null) setOpenMenu(menu.label);
              }}
            >
              {menu.label}
            </button>
            {openMenu === menu.label && (
              <div style={styles.dropdown}>
                {menu.items.map((item, i) =>
                  item.separator ? (
                    <div key={`sep-${i}`} style={styles.separator} />
                  ) : (
                    <button
                      key={item.label}
                      style={{
                        ...styles.dropdownItem,
                        ...(item.disabled ? styles.dropdownItemDisabled : {}),
                      }}
                      disabled={item.disabled}
                      onClick={async () => {
                        await item.action();
                        closeMenu();
                      }}
                      onMouseEnter={(e) => {
                        if (!item.disabled)
                          (e.currentTarget as HTMLElement).style.background =
                            "#30363d";
                      }}
                      onMouseLeave={(e) => {
                        (e.currentTarget as HTMLElement).style.background =
                          "transparent";
                      }}
                    >
                      {item.label}
                    </button>
                  ),
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      <div style={styles.rightSection}>
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
        <span style={styles.tick}>Tick: {tick}</span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  toolbar: {
    display: "flex",
    alignItems: "center",
    gap: 16,
    padding: "4px 16px",
    background: "#161b22",
    borderBottom: "1px solid #30363d",
    flexShrink: 0,
    height: 36,
  },
  brand: {
    fontSize: 18,
    fontWeight: "bold",
    color: "#58a6ff",
    marginRight: 8,
  },
  menuBar: {
    display: "flex",
    alignItems: "center",
    gap: 0,
  },
  menuContainer: {
    position: "relative" as const,
  },
  menuButton: {
    padding: "4px 12px",
    background: "transparent",
    color: "#c9d1d9",
    border: "1px solid transparent",
    borderRadius: 4,
    cursor: "pointer",
    fontSize: 13,
    lineHeight: "20px",
  },
  menuButtonActive: {
    background: "#21262d",
    border: "1px solid #30363d",
    borderBottomColor: "#161b22",
    borderBottomLeftRadius: 0,
    borderBottomRightRadius: 0,
  },
  dropdown: {
    position: "absolute" as const,
    top: "100%",
    left: 0,
    minWidth: 180,
    background: "#1c2128",
    border: "1px solid #30363d",
    borderRadius: "0 6px 6px 6px",
    padding: "4px 0",
    zIndex: 1000,
    boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
  },
  dropdownItem: {
    display: "block",
    width: "100%",
    padding: "6px 16px",
    background: "transparent",
    color: "#c9d1d9",
    border: "none",
    cursor: "pointer",
    fontSize: 13,
    textAlign: "left" as const,
    lineHeight: "20px",
  },
  dropdownItemDisabled: {
    color: "#484f58",
    cursor: "default",
  },
  separator: {
    height: 1,
    background: "#30363d",
    margin: "4px 0",
  },
  rightSection: {
    display: "flex",
    alignItems: "center",
    gap: 16,
    marginLeft: "auto",
  },
  controls: {
    display: "flex",
    alignItems: "center",
    gap: 8,
  },
  tick: {
    color: "#8b949e",
    fontSize: 13,
    fontFamily: "monospace",
  },
  label: {
    color: "#8b949e",
    fontSize: 12,
  },
};

import { useSelectionStore } from "../../store/selectionStore.ts";
import { useTopologyStore } from "../../store/topologyStore.ts";
import type { Router } from "../../types/models.ts";
import { formatBps } from "../../utils/formatting.ts";

export default function RouterPanel() {
  const { selectedNodeId, selectedNodeType } = useSelectionStore();
  const { autonomousSystems, standaloneRouters } = useTopologyStore();

  if (selectedNodeType !== "router" || !selectedNodeId) {
    return <div style={styles.empty}>Select a router to view details</div>;
  }

  // Find the router (check standalone first, then AS-bound)
  let router: Router | null = null;
  let asName = "";

  if (standaloneRouters[selectedNodeId]) {
    router = standaloneRouters[selectedNodeId]!;
    asName = "Standalone";
  } else {
    for (const as_ of Object.values(autonomousSystems)) {
      if (as_.routers[selectedNodeId]) {
        router = as_.routers[selectedNodeId]!;
        asName = `${as_.name} (AS${as_.asn})`;
        break;
      }
    }
  }

  if (!router) {
    return <div style={styles.empty}>Router not found</div>;
  }

  const interfaces = Object.values(router.interfaces);

  return (
    <div style={styles.panel}>
      <h3 style={styles.title}>{router.name}</h3>
      <div style={styles.meta}>
        <div>Router ID: {router.router_id_ip}</div>
        <div>AS: {asName}</div>
        <div>
          OSPF:{" "}
          {router.ospf_process ? "Enabled" : "Disabled"}
        </div>
        <div>
          BGP:{" "}
          {router.bgp_process
            ? `Enabled (AS${router.bgp_process.local_asn})`
            : "Disabled"}
        </div>
      </div>

      <h4 style={styles.subtitle}>
        Interfaces ({interfaces.length})
      </h4>
      <table style={styles.table}>
        <thead>
          <tr>
            <th style={styles.th}>Name</th>
            <th style={styles.th}>IP</th>
            <th style={styles.th}>Cost</th>
            <th style={styles.th}>BW</th>
            <th style={styles.th}>Status</th>
          </tr>
        </thead>
        <tbody>
          {interfaces.map((iface) => (
            <tr key={iface.id}>
              <td style={styles.td}>
                {iface.id.replace(router!.id + "-", "")}
              </td>
              <td style={styles.td}>{iface.ip_address}</td>
              <td style={styles.td}>{iface.cost}</td>
              <td style={styles.td}>
                {iface.bandwidth ? formatBps(iface.bandwidth) : "-"}
              </td>
              <td style={styles.td}>
                <span
                  style={{
                    color: iface.is_up ? "#48bb78" : "#f56565",
                  }}
                >
                  {iface.is_up ? "UP" : "DOWN"}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h4 style={styles.subtitle}>
        RIB ({Object.keys(router.rib).length} routes)
      </h4>
      <table style={styles.table}>
        <thead>
          <tr>
            <th style={styles.th}>Prefix</th>
            <th style={styles.th}>Next Hop</th>
            <th style={styles.th}>Protocol</th>
            <th style={styles.th}>Metric</th>
            <th style={styles.th}>AD</th>
          </tr>
        </thead>
        <tbody>
          {Object.values(router.rib).map((entry) => (
            <tr key={entry.prefix}>
              <td style={styles.td}>{entry.prefix}</td>
              <td style={styles.td}>{entry.next_hop}</td>
              <td style={styles.td}>{entry.protocol}</td>
              <td style={styles.td}>{entry.metric}</td>
              <td style={styles.td}>{entry.admin_distance}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {router.bgp_process && (
        <>
          <h4 style={styles.subtitle}>BGP Neighbors</h4>
          <table style={styles.table}>
            <thead>
              <tr>
                <th style={styles.th}>Neighbor</th>
                <th style={styles.th}>Remote AS</th>
                <th style={styles.th}>State</th>
                <th style={styles.th}>Type</th>
              </tr>
            </thead>
            <tbody>
              {Object.values(router.bgp_process.neighbors).map((n) => (
                <tr key={n.neighbor_ip}>
                  <td style={styles.td}>{n.neighbor_ip}</td>
                  <td style={styles.td}>{n.remote_asn}</td>
                  <td style={styles.td}>
                    <span
                      style={{
                        color:
                          n.state === "Established"
                            ? "#48bb78"
                            : "#ecc94b",
                      }}
                    >
                      {n.state}
                    </span>
                  </td>
                  <td style={styles.td}>{n.is_ebgp ? "eBGP" : "iBGP"}</td>
                </tr>
              ))}
            </tbody>
          </table>

          <h4 style={styles.subtitle}>BGP Best Routes</h4>
          <table style={styles.table}>
            <thead>
              <tr>
                <th style={styles.th}>Prefix</th>
                <th style={styles.th}>Next Hop</th>
                <th style={styles.th}>AS Path</th>
                <th style={styles.th}>LP</th>
                <th style={styles.th}>MED</th>
              </tr>
            </thead>
            <tbody>
              {Object.values(router.bgp_process.best_routes).map((r) => (
                <tr key={r.prefix}>
                  <td style={styles.td}>{r.prefix}</td>
                  <td style={styles.td}>{r.attributes.next_hop}</td>
                  <td style={styles.td}>
                    {r.attributes.as_path.join(" ")}
                  </td>
                  <td style={styles.td}>{r.attributes.local_pref}</td>
                  <td style={styles.td}>{r.attributes.med}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {router.traffic_generators.length > 0 && (
        <>
          <h4 style={styles.subtitle}>Traffic Generators</h4>
          <table style={styles.table}>
            <thead>
              <tr>
                <th style={styles.th}>Dest</th>
                <th style={styles.th}>Rate</th>
                <th style={styles.th}>Active</th>
              </tr>
            </thead>
            <tbody>
              {router.traffic_generators.map((gen) => (
                <tr key={gen.id}>
                  <td style={styles.td}>{gen.dest_prefix}</td>
                  <td style={styles.td}>{formatBps(gen.rate_bps)}</td>
                  <td style={styles.td}>
                    <span
                      style={{
                        color: gen.is_active ? "#48bb78" : "#8b949e",
                      }}
                    >
                      {gen.is_active ? "Yes" : "No"}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  panel: {
    padding: 16,
    overflowY: "auto",
    height: "100%",
  },
  empty: {
    padding: 24,
    color: "#8b949e",
    textAlign: "center",
  },
  title: {
    margin: 0,
    fontSize: 16,
    color: "#e2e8f0",
  },
  subtitle: {
    marginTop: 16,
    marginBottom: 4,
    fontSize: 13,
    color: "#a0aec0",
    borderBottom: "1px solid #30363d",
    paddingBottom: 4,
  },
  meta: {
    marginTop: 8,
    fontSize: 12,
    color: "#8b949e",
    lineHeight: 1.8,
  },
  table: {
    width: "100%",
    borderCollapse: "collapse",
    fontSize: 11,
    fontFamily: "monospace",
  },
  th: {
    textAlign: "left",
    padding: "4px 6px",
    borderBottom: "1px solid #30363d",
    color: "#8b949e",
    fontWeight: "normal",
  },
  td: {
    padding: "3px 6px",
    borderBottom: "1px solid #21262d",
    color: "#c9d1d9",
  },
};

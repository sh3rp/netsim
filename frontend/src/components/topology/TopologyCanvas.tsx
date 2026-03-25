import { useEffect, useRef, useCallback, useState } from "react";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import { useTopologyStore, getAllRouters } from "../../store/topologyStore.ts";
import { useSelectionStore } from "../../store/selectionStore.ts";
import * as api from "../../api/client.ts";
import {
  cytoscapeStylesheet,
  defaultLayout,
  getAsColor,
} from "./cytoscapeConfig.ts";
import { utilizationColor } from "../../utils/formatting.ts";

// Per-AS padding overrides survive topology refreshes
const asPaddingMap = new Map<string, number>();
const DEFAULT_PADDING = 30;

// Auto-increment counter for generating unique interface IPs
let linkSubnetCounter = 100;

export default function TopologyCanvas() {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);

  const { autonomousSystems, standaloneRouters, links, linkUtilization } =
    useTopologyStore();
  const { selectRouter, selectAS, selectLink, clearSelection } =
    useSelectionStore();

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    items: { label: string; action: () => void; color?: string }[];
  } | null>(null);

  // Connection-drawing state
  const connectingRef = useRef<{
    sourceRouterId: string;
    sourceRouterName: string;
  } | null>(null);
  const [connecting, setConnecting] = useState(false);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => closeContextMenu();
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [contextMenu, closeContextMenu]);

  const refreshTopology = useCallback(async () => {
    const snapshot = await api.getTopologySnapshot();
    useTopologyStore.getState().setTopology(snapshot);
  }, []);

  /** Connect two routers: create interfaces on each + a link between them */
  const connectRouters = useCallback(
    async (routerAId: string, routerBId: string) => {
      const snapshot = await api.getTopologySnapshot();
      const allRouters = [
        ...Object.values(snapshot.autonomous_systems).flatMap(
          (as_: any) => Object.values(as_.routers) as any[],
        ),
        ...Object.values(snapshot.standalone_routers),
      ] as any[];

      const routerA = allRouters.find((r: any) => r.id === routerAId);
      const routerB = allRouters.find((r: any) => r.id === routerBId);
      if (!routerA || !routerB) return;

      const nextEthIndex = (router: any) => {
        const existing = Object.values(router.interfaces) as any[];
        let max = -1;
        for (const iface of existing) {
          const match = iface.id?.match(/eth(\d+)/);
          if (match) max = Math.max(max, parseInt(match[1], 10));
        }
        return max + 1;
      };

      const ethA = nextEthIndex(routerA);
      const ethB = nextEthIndex(routerB);
      const subnet = linkSubnetCounter++;
      const ipA = `10.${Math.floor(subnet / 256)}.${subnet % 256}.1/30`;
      const ipB = `10.${Math.floor(subnet / 256)}.${subnet % 256}.2/30`;

      const ifaceA = await api.addInterface(
        routerAId, `eth${ethA}`, ipA, 1000000000, 10,
      );
      const ifaceB = await api.addInterface(
        routerBId, `eth${ethB}`, ipB, 1000000000, 10,
      );
      await api.createLink(ifaceA.id, ifaceB.id, 1000000000, 1);
      await refreshTopology();
    },
    [refreshTopology],
  );

  // Build Cytoscape elements from topology
  const buildElements = useCallback((): ElementDefinition[] => {
    const elements: ElementDefinition[] = [];
    let asIndex = 0;

    for (const [asId, as_] of Object.entries(autonomousSystems)) {
      const padding = asPaddingMap.get(asId) ?? DEFAULT_PADDING;

      elements.push({
        data: {
          id: asId,
          label: `${as_.name} (AS${as_.asn})`,
        },
        classes: "as-group",
        grabbable: true,
        style: {
          "border-color": getAsColor(asIndex),
          padding: padding,
        } as any,
      });

      for (const [routerId, router] of Object.entries(as_.routers)) {
        const classes = ["router"];
        if (router.bgp_process) classes.push("bgp-enabled");

        elements.push({
          data: {
            id: routerId,
            label: router.name,
            parent: asId,
            routerData: router,
          },
          position: { x: router.position[0], y: router.position[1] },
          classes: classes.join(" "),
          grabbable: true,
        });
      }
      asIndex++;
    }

    for (const [routerId, router] of Object.entries(standaloneRouters)) {
      const classes = ["router", "standalone"];
      if (router.bgp_process) classes.push("bgp-enabled");

      elements.push({
        data: {
          id: routerId,
          label: router.name,
          routerData: router,
        },
        position: { x: router.position[0], y: router.position[1] },
        classes: classes.join(" "),
        grabbable: true,
      });
    }

    const allRouters = getAllRouters(autonomousSystems, standaloneRouters);
    for (const [linkId, link] of Object.entries(links)) {
      const routerA = allRouters.find((r) =>
        Object.keys(r.interfaces).includes(link.interface_a_id),
      );
      const routerB = allRouters.find((r) =>
        Object.keys(r.interfaces).includes(link.interface_b_id),
      );

      if (routerA && routerB) {
        const util = linkUtilization[linkId] ?? 0;
        const classes = [];
        if (!link.is_up) classes.push("link-down");
        if (util > 0.01) classes.push("traffic-active");

        elements.push({
          data: {
            id: linkId,
            source: routerA.id,
            target: routerB.id,
            label: link.is_up ? "" : "DOWN",
          },
          classes: classes.join(" "),
          style: {
            "line-color": link.is_up ? utilizationColor(util) : "#e53e3e",
            width: Math.max(2, Math.min(8, 2 + util * 6)),
          } as any,
        });
      }
    }

    return elements;
  }, [autonomousSystems, standaloneRouters, links, linkUtilization]);

  // Initialize Cytoscape
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      style: [
        ...cytoscapeStylesheet,
        {
          selector: "edge.connect-ghost",
          style: {
            width: 2,
            "line-color": "#58a6ff",
            "line-style": "dashed",
            "line-dash-pattern": [8, 4],
            "target-arrow-shape": "triangle",
            "target-arrow-color": "#58a6ff",
            "curve-style": "bezier",
          } as any,
        },
        {
          selector: "node.connect-target",
          style: {
            "border-color": "#58a6ff",
            "border-width": 3,
          },
        },
      ],
      layout: defaultLayout,
      elements: buildElements(),
      userZoomingEnabled: true,
      userPanningEnabled: true,
      boxSelectionEnabled: false,
    });

    // Ghost node for connection line endpoint
    cy.add({
      data: { id: "__connect_ghost_target" },
      position: { x: 0, y: 0 },
      grabbable: false,
      selectable: false,
    });
    cy.getElementById("__connect_ghost_target").style({
      width: 1, height: 1,
      "background-opacity": 0, "border-width": 0, label: "",
    } as any);
    cy.getElementById("__connect_ghost_target").style("display", "none");

    // --- Edge-resize for AS compound nodes ---
    // On grab, check if the grab position is near the bounding box edge.
    // If so, cancel the grab (ungrabify), enter resize mode, and
    // handle resize via Cytoscape's own mousemove/mouseup events.
    const EDGE_ZONE = 18;
    let resizeInfo: {
      nodeId: string;
      edge: string;
      startX: number;
      startY: number;
      startPadding: number;
    } | null = null;

    const CURSOR_MAP: Record<string, string> = {
      n: "ns-resize", s: "ns-resize", e: "ew-resize", w: "ew-resize",
      nw: "nwse-resize", se: "nwse-resize", ne: "nesw-resize", sw: "nesw-resize",
    };

    function detectEdge(bb: { x1: number; x2: number; y1: number; y2: number }, mx: number, my: number): string | null {
      if (mx < bb.x1 - EDGE_ZONE || mx > bb.x2 + EDGE_ZONE ||
          my < bb.y1 - EDGE_ZONE || my > bb.y2 + EDGE_ZONE) return null;
      const nearL = Math.abs(mx - bb.x1) < EDGE_ZONE;
      const nearR = Math.abs(mx - bb.x2) < EDGE_ZONE;
      const nearT = Math.abs(my - bb.y1) < EDGE_ZONE;
      const nearB = Math.abs(my - bb.y2) < EDGE_ZONE;
      if (nearT && nearL) return "nw";
      if (nearT && nearR) return "ne";
      if (nearB && nearL) return "sw";
      if (nearB && nearR) return "se";
      if (nearT) return "n";
      if (nearB) return "s";
      if (nearL) return "w";
      if (nearR) return "e";
      return null;
    }

    // When an AS group is grabbed, check if it was on an edge
    cy.on("grab", "node.as-group", (evt) => {
      const node = evt.target;
      const pos = evt.position;
      if (!pos) return;
      const bb = node.boundingBox();
      const edge = detectEdge(bb, pos.x, pos.y);
      if (edge) {
        // Cancel the drag - switch to resize mode
        node.ungrabify();
        // Release will be triggered, re-grabify after
        const currentPadding = asPaddingMap.get(node.id()) ?? DEFAULT_PADDING;
        resizeInfo = {
          nodeId: node.id(),
          edge,
          startX: pos.x,
          startY: pos.y,
          startPadding: currentPadding,
        };
        cy.userPanningEnabled(false);
        cy.userZoomingEnabled(false);
        if (containerRef.current) containerRef.current.style.cursor = CURSOR_MAP[edge] || "";
      }
    });

    // Track resize via mousemove on the core
    cy.on("vmousemove", (evt) => {
      const pos = evt.position;
      if (!pos) return;

      if (resizeInfo) {
        const { nodeId, edge, startX, startY, startPadding } = resizeInfo;
        const dx = pos.x - startX;
        const dy = pos.y - startY;

        let delta = 0;
        if (edge.includes("e")) delta = Math.max(delta, dx);
        if (edge.includes("w")) delta = Math.max(delta, -dx);
        if (edge.includes("s")) delta = Math.max(delta, dy);
        if (edge.includes("n")) delta = Math.max(delta, -dy);

        const newPadding = Math.max(10, startPadding + delta);
        asPaddingMap.set(nodeId, newPadding);
        const node = cy.getElementById(nodeId);
        if (node.length > 0) {
          node.style("padding" as any, newPadding);
        }
        return;
      }

      // Hover: show resize cursor when near edges
      if (containerRef.current) {
        let cursor = "";
        const compoundNodes = cy.nodes(".as-group");
        for (let i = 0; i < compoundNodes.length; i++) {
          const bb = compoundNodes[i].boundingBox();
          const edge = detectEdge(bb, pos.x, pos.y);
          if (edge) { cursor = CURSOR_MAP[edge] || ""; break; }
        }
        containerRef.current.style.cursor = cursor;
      }
    });

    // End resize on mouseup
    cy.on("vmouseup", () => {
      if (!resizeInfo) return;
      const node = cy.getElementById(resizeInfo.nodeId);
      if (node.length > 0) node.grabify();
      resizeInfo = null;
      cy.userPanningEnabled(true);
      cy.userZoomingEnabled(true);
      if (containerRef.current) containerRef.current.style.cursor = "";
    });

    // Selection handlers
    cy.on("tap", "node.router", (evt) => {
      const node = evt.target;
      if (connectingRef.current) {
        const source = connectingRef.current;
        const targetId = node.id();
        if (targetId !== source.sourceRouterId) {
          connectRouters(source.sourceRouterId, targetId);
        }
        cancelConnect(cy);
        return;
      }
      const parentId = node.data("parent");
      selectRouter(node.id(), parentId);
    });

    cy.on("tap", "node.as-group", (evt) => {
      if (connectingRef.current) { cancelConnect(cy); return; }
      selectAS(evt.target.id());
    });

    cy.on("tap", "edge", (evt) => {
      if (connectingRef.current) { cancelConnect(cy); return; }
      selectLink(evt.target.id());
    });

    cy.on("tap", (evt) => {
      setContextMenu(null);
      if (evt.target === cy) {
        if (connectingRef.current) { cancelConnect(cy); return; }
        clearSelection();
      }
    });

    // Connection line follows mouse
    cy.on("mousemove", (evt) => {
      if (!connectingRef.current) return;
      const pos = evt.position;
      if (!pos) return;
      const ghost = cy.getElementById("__connect_ghost_target");
      ghost.position(pos);
      cy.nodes(".connect-target").removeClass("connect-target");
      const target = evt.target;
      if (
        target !== cy &&
        target.isNode() &&
        target.hasClass("router") &&
        target.id() !== connectingRef.current.sourceRouterId
      ) {
        target.addClass("connect-target");
      }
    });

    // Right-click context menu on routers
    cy.on("cxttap", "node.router", (evt) => {
      if (connectingRef.current) cancelConnect(cy);
      const node = evt.target;
      const id = node.id();
      const label = node.data("label") || id;
      const oe = evt.originalEvent as MouseEvent;
      setContextMenu({
        x: oe.clientX,
        y: oe.clientY,
        items: [
          {
            label: "Connect",
            color: "#58a6ff",
            action: () => { startConnect(cy, id, label); },
          },
          {
            label: "Delete",
            color: "#f85149",
            action: async () => {
              await api.deleteRouter(id);
              await refreshTopology();
              clearSelection();
            },
          },
        ],
      });
    });

    // Right-click context menu on AS groups
    cy.on("cxttap", "node.as-group", (evt) => {
      if (connectingRef.current) cancelConnect(cy);
      const node = evt.target;
      const id = node.id();
      const label = node.data("label") || id;
      const oe = evt.originalEvent as MouseEvent;
      setContextMenu({
        x: oe.clientX,
        y: oe.clientY,
        items: [
          {
            label: `Delete "${label}"`,
            color: "#f85149",
            action: async () => {
              await api.deleteAS(id);
              await refreshTopology();
              clearSelection();
            },
          },
        ],
      });
    });

    // Right-click context menu on links
    cy.on("cxttap", "edge", (evt) => {
      if (connectingRef.current) cancelConnect(cy);
      const edge = evt.target;
      const id = edge.id();
      if (id === "__connect_ghost_edge") return;
      const oe = evt.originalEvent as MouseEvent;
      setContextMenu({
        x: oe.clientX,
        y: oe.clientY,
        items: [
          {
            label: "Delete Link",
            color: "#f85149",
            action: async () => {
              await api.deleteLink(id);
              await refreshTopology();
              clearSelection();
            },
          },
        ],
      });
    });

    // Suppress browser context menu
    containerRef.current.addEventListener("contextmenu", (e) => e.preventDefault());

    // Escape key cancels connect mode
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && connectingRef.current) cancelConnect(cy);
    };
    document.addEventListener("keydown", handleKeyDown);

    cyRef.current = cy;

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
  }, []); // Only init once

  function startConnect(cy: Core, sourceRouterId: string, sourceName: string) {
    connectingRef.current = { sourceRouterId, sourceRouterName: sourceName };
    setConnecting(true);
    const sourceNode = cy.getElementById(sourceRouterId);
    const ghost = cy.getElementById("__connect_ghost_target");
    ghost.style("display", "element");
    ghost.position(sourceNode.position());
    cy.add({
      data: {
        id: "__connect_ghost_edge",
        source: sourceRouterId,
        target: "__connect_ghost_target",
      },
      classes: "connect-ghost",
    });
  }

  function cancelConnect(cy: Core) {
    connectingRef.current = null;
    setConnecting(false);
    const ghostEdge = cy.getElementById("__connect_ghost_edge");
    if (ghostEdge.length > 0) ghostEdge.remove();
    const ghostNode = cy.getElementById("__connect_ghost_target");
    ghostNode.style("display", "none");
    cy.nodes(".connect-target").removeClass("connect-target");
  }

  // Update elements when data changes
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const elements = buildElements();

    cy.batch(() => {
      const newIds = new Set(elements.map((e) => e.data.id!));
      cy.elements().forEach((ele) => {
        const id = ele.id();
        if (
          !newIds.has(id) &&
          id !== "__connect_ghost_target" &&
          id !== "__connect_ghost_edge"
        ) {
          ele.remove();
        }
      });

      for (const elem of elements) {
        const existing = cy.getElementById(elem.data.id!);
        if (existing.length > 0) {
          existing.data(elem.data);
          if (elem.style) existing.style(elem.style as any);
          if (elem.classes) existing.classes(elem.classes);
        } else {
          cy.add(elem);
        }
      }
    });
  }, [buildElements]);

  return (
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <div
        ref={containerRef}
        style={{
          width: "100%",
          height: "100%",
          background: "#0d1117",
          borderRadius: "8px",
        }}
      />
      {connecting && (
        <div
          style={{
            position: "absolute",
            top: 8,
            left: "50%",
            transform: "translateX(-50%)",
            background: "#1c2128",
            border: "1px solid #58a6ff",
            borderRadius: 6,
            padding: "6px 16px",
            color: "#58a6ff",
            fontSize: 13,
            zIndex: 1500,
            pointerEvents: "none",
          }}
        >
          Click a target router to connect — Esc to cancel
        </div>
      )}
      {contextMenu && (
        <div
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            minWidth: 160,
            background: "#1c2128",
            border: "1px solid #30363d",
            borderRadius: 6,
            padding: "4px 0",
            zIndex: 2000,
            boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
          }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {contextMenu.items.map((item) => (
            <button
              key={item.label}
              style={{
                display: "block",
                width: "100%",
                padding: "6px 16px",
                background: "transparent",
                color: item.color || "#c9d1d9",
                border: "none",
                cursor: "pointer",
                fontSize: 13,
                textAlign: "left",
                lineHeight: "20px",
              }}
              onMouseEnter={(e) => {
                (e.currentTarget as HTMLElement).style.background = "#30363d";
              }}
              onMouseLeave={(e) => {
                (e.currentTarget as HTMLElement).style.background = "transparent";
              }}
              onClick={() => {
                closeContextMenu();
                item.action();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

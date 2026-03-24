import { useEffect, useRef, useCallback } from "react";
import cytoscape, { type Core, type ElementDefinition } from "cytoscape";
import { useTopologyStore, getAllRouters } from "../../store/topologyStore.ts";
import { useSelectionStore } from "../../store/selectionStore.ts";
import {
  cytoscapeStylesheet,
  defaultLayout,
  getAsColor,
} from "./cytoscapeConfig.ts";
import { utilizationColor } from "../../utils/formatting.ts";

export default function TopologyCanvas() {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | null>(null);

  const { autonomousSystems, links, linkUtilization } = useTopologyStore();
  const { selectRouter, selectAS, selectLink, clearSelection } =
    useSelectionStore();

  // Build Cytoscape elements from topology
  const buildElements = useCallback((): ElementDefinition[] => {
    const elements: ElementDefinition[] = [];
    let asIndex = 0;

    for (const [asId, as_] of Object.entries(autonomousSystems)) {
      // AS compound node
      elements.push({
        data: {
          id: asId,
          label: `${as_.name} (AS${as_.asn})`,
        },
        classes: "as-group",
        style: {
          "border-color": getAsColor(asIndex),
        },
      });

      // Router nodes
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
        });
      }
      asIndex++;
    }

    // Links
    const allRouters = getAllRouters(autonomousSystems);
    for (const [linkId, link] of Object.entries(links)) {
      // Find router IDs for each interface
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
  }, [autonomousSystems, links, linkUtilization]);

  // Initialize Cytoscape
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      style: cytoscapeStylesheet,
      layout: defaultLayout,
      elements: buildElements(),
      userZoomingEnabled: true,
      userPanningEnabled: true,
      boxSelectionEnabled: false,
    });

    // Event handlers
    cy.on("tap", "node.router", (evt) => {
      const node = evt.target;
      const parentId = node.data("parent");
      selectRouter(node.id(), parentId);
    });

    cy.on("tap", "node.as-group", (evt) => {
      selectAS(evt.target.id());
    });

    cy.on("tap", "edge", (evt) => {
      selectLink(evt.target.id());
    });

    cy.on("tap", (evt) => {
      if (evt.target === cy) {
        clearSelection();
      }
    });

    // Allow dragging router nodes
    cy.on("dragfree", "node.router", (_evt) => {
      // Position is persisted in Cytoscape; could sync back to server
    });

    cyRef.current = cy;

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, []); // Only init once

  // Update elements when data changes
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const elements = buildElements();

    cy.batch(() => {
      // Remove elements that no longer exist
      const newIds = new Set(elements.map((e) => e.data.id!));
      cy.elements().forEach((ele) => {
        if (!newIds.has(ele.id())) {
          ele.remove();
        }
      });

      // Update or add elements
      for (const elem of elements) {
        const existing = cy.getElementById(elem.data.id!);
        if (existing.length > 0) {
          // Update data and style
          existing.data(elem.data);
          if (elem.style) {
            existing.style(elem.style as any);
          }
          if (elem.classes) {
            existing.classes(elem.classes);
          }
        } else {
          cy.add(elem);
        }
      }
    });
  }, [buildElements]);

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        background: "#0d1117",
        borderRadius: "8px",
      }}
    />
  );
}

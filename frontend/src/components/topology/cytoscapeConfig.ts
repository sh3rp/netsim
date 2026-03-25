import type { Stylesheet, LayoutOptions } from "cytoscape";

const AS_COLORS = [
  "#2d3748", "#2c5282", "#285e61", "#553c9a", "#744210",
  "#702459", "#1a365d", "#22543d",
];

export function getAsColor(index: number): string {
  return AS_COLORS[index % AS_COLORS.length]!;
}

export const cytoscapeStylesheet: Stylesheet[] = [
  // AS compound nodes
  {
    selector: "node.as-group",
    style: {
      "background-color": "#1a202c",
      "background-opacity": 0.6,
      "border-color": "#4a5568",
      "border-width": 2,
      label: "data(label)",
      "text-valign": "top",
      "text-halign": "center",
      color: "#a0aec0",
      "font-size": "14px",
      "font-weight": "bold",
      padding: "30px" as any,
      shape: "roundrectangle",
    },
  },
  // Router nodes
  {
    selector: "node.router",
    style: {
      "background-color": "#4299e1",
      label: "data(label)",
      "text-valign": "bottom",
      "text-halign": "center",
      color: "#e2e8f0",
      "font-size": "11px",
      width: 35,
      height: 35,
      "border-width": 2,
      "border-color": "#63b3ed",
      shape: "ellipse",
    },
  },
  // Standalone router (no AS)
  {
    selector: "node.router.standalone",
    style: {
      "background-color": "#38b2ac",
      "border-color": "#4fd1c5",
      "border-style": "dashed",
    },
  },
  // Router with BGP enabled
  {
    selector: "node.router.bgp-enabled",
    style: {
      "background-color": "#9f7aea",
      "border-color": "#b794f4",
    },
  },
  // Selected node
  {
    selector: "node:selected",
    style: {
      "border-color": "#fbd38d",
      "border-width": 3,
      "background-color": "#ed8936",
    },
  },
  // Links
  {
    selector: "edge",
    style: {
      width: 3,
      "line-color": "#4a5568",
      "curve-style": "bezier",
      label: "data(label)",
      "font-size": "9px",
      color: "#718096",
      "text-rotation": "autorotate",
      "text-margin-y": -10,
    },
  },
  // Link down
  {
    selector: "edge.link-down",
    style: {
      "line-style": "dashed",
      "line-color": "#e53e3e",
      "line-dash-pattern": [6, 3],
    },
  },
  // Selected edge
  {
    selector: "edge:selected",
    style: {
      "line-color": "#fbd38d",
      width: 4,
    },
  },
  // Traffic flow highlight
  {
    selector: "edge.traffic-active",
    style: {
      "line-color": "#48bb78",
      width: 4,
    },
  },
];

export const defaultLayout: LayoutOptions = {
  name: "preset",
};

export const autoLayout: LayoutOptions = {
  name: "cose",
  animate: true,
  animationDuration: 500,
  nodeRepulsion: () => 8000,
  idealEdgeLength: () => 120,
  gravity: 0.25,
} as any;

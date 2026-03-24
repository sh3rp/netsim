# NetSim

A real-time network simulator for designing and simulating network topologies with routing protocols, traffic flows, and route policies.

## Features

- **Topology Design** — Create autonomous systems, routers, interfaces, and links through an interactive canvas
- **OSPF Simulation** — Link-state routing with LSA flooding, SPF (Dijkstra), neighbor state machine, and multi-area support
- **BGP Simulation** — Full FSM, best-path selection (RFC 4271), iBGP/eBGP peering, Adj-RIBs, and Loc-RIB
- **Traffic Simulation** — Define traffic generators, trace flows through the network via FIB lookups, and visualize per-link utilization
- **Route Policies** — Custom DSL for BGP import/export filtering with match conditions and set actions
- **Real-Time Visualization** — Cytoscape.js topology graph with live link utilization coloring, WebSocket tick updates, and interactive pan/zoom/drag
- **Save/Load** — Export and import full topology snapshots as JSON

## Architecture

```
Frontend (React + TypeScript + Vite)         Backend (Rust + Actix-web + Tokio)
┌──────────────────────────────┐             ┌──────────────────────────────────┐
│  Cytoscape Topology Canvas   │  REST/WS    │  REST API (/api/v1/*)            │
│  Router & Link Detail Panels │◄───────────►│  WebSocket (/ws) tick broadcast  │
│  Zustand State Stores        │  :3000→:8080│  Simulation Engine (tick-based)  │
│  Toolbar & Status Bar        │             │  OSPF / BGP / Traffic / Policies │
└──────────────────────────────┘             └──────────────────────────────────┘
```

The frontend runs on port 3000 and proxies API/WebSocket requests to the backend on port 8080. The simulation engine runs a configurable tick loop (default 200ms), processing protocol events and broadcasting state updates to all connected clients.

## Getting Started

### Prerequisites

- **Rust** (stable toolchain)
- **Bun** (or Node.js 18+)

### Run

```bash
# Start the backend
cd backend
RUST_LOG=info cargo run

# In another terminal, start the frontend
cd frontend
bun install
bun run dev
```

Open http://localhost:3000 in your browser. Click **Load Sample** in the toolbar to load a pre-built two-AS topology, then **Start** to begin the simulation.

### Build for Production

```bash
cd frontend
bun run build   # outputs to dist/

cd ../backend
cargo build --release
```

## UI Overview

The interface is a dark-themed split layout:

- **Toolbar** (top) — Add AS/routers/links, start/stop simulation, adjust tick rate, save/load topologies
- **Topology Canvas** (left) — Interactive graph showing autonomous systems as groups, routers as nodes, and links as edges. Link color reflects utilization (green → yellow → red). Failed links appear in red.
- **Detail Panel** (right) — Context-sensitive view that shows:
  - **Router details** — interfaces, RIB entries, OSPF neighbors, BGP peers, BGP best routes, traffic generators
  - **Link details** — connected routers, bandwidth, delay, current load/utilization, and a button to fail/restore the link
- **Status Bar** (bottom) — Current simulation tick and state

## Route Policy DSL

Policies filter and modify BGP routes on import/export. Example:

```
policy "prefer-customer" {
  term 1 {
    match community "65001:100"
    set local-pref 150
    accept
  }
  default reject
}
```

### Match Conditions

| Condition | Description |
|-----------|-------------|
| `match prefix "10.0.0.0/8"` | Match destination prefix |
| `match community "65001:100"` | Match BGP community |
| `match as-path "^65002_"` | Match AS path (prefix/suffix/contains) |

### Set Actions

| Action | Description |
|--------|-------------|
| `set local-pref <n>` | Set local preference |
| `set med <n>` | Set multi-exit discriminator |
| `prepend-as <asn> <count>` | Prepend ASN to AS path |
| `add-community "value"` | Add a BGP community |
| `remove-community "value"` | Remove a BGP community |

Each term ends with `accept` or `reject`. Policies have a `default accept|reject` fallthrough.

## API

All endpoints are under `/api/v1`. Key groups:

| Group | Endpoints | Description |
|-------|-----------|-------------|
| Topology | `/topology/as`, `/topology/routers/{id}`, `/topology/links` | CRUD for AS, routers, interfaces, links |
| OSPF | `/ospf/{router_id}/lsdb`, `neighbors`, `routes` | Query OSPF state per router |
| BGP | `/bgp/{router_id}/enable`, `neighbors`, `routes` | Manage BGP sessions and view routes |
| Traffic | `/traffic/generators`, `flows`, `link-utilization` | Traffic generators and flow tracing |
| Simulation | `/simulation/start`, `stop`, `step`, `state`, `tick-rate` | Control the simulation engine |
| Policies | `/policies` | Create and manage route policies |
| WebSocket | `/ws` | Real-time tick update stream |

## Tech Stack

- **Backend:** Rust, Actix-web, Tokio, petgraph (Dijkstra SPF), serde
- **Frontend:** React 19, TypeScript, Vite, Cytoscape.js, Zustand, TanStack Table
- **Runtime:** Bun (package manager / dev server)

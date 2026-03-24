# NetSim — Agent Guide

## Project Overview

NetSim is a real-time network simulator with a Rust backend (Actix-web) and React TypeScript frontend. It supports topology design, OSPF/BGP routing protocol simulation, traffic flow tracing, and route policies via a custom DSL. The simulation runs tick-based with WebSocket updates to the UI.

## Architecture

```
Frontend (React 19 + TypeScript + Vite + Cytoscape)
    ↕  HTTP REST + WebSocket (port 3000 → proxy → 8080)
Backend (Rust + Actix-web 4 + Tokio)
    ├── api/       — HTTP handlers & WebSocket
    ├── engine/    — Simulation core (OSPF, BGP, traffic, policies, events)
    └── persistence/ — JSON save/load
```

State is held in `Arc<RwLock<SimulationState>>` shared across Actix handlers and the background simulation loop.

## Key Directories & Files

### Backend (`backend/`)
| Path | Purpose |
|------|---------|
| `src/main.rs` | Server startup, CORS, route registration, simulation loop spawn |
| `src/engine/models.rs` | All data structures (Router, Interface, Link, OSPF/BGP processes) |
| `src/engine/simulation.rs` | `SimulationEngine::step()`, event processing, tick loop |
| `src/engine/ospf.rs` | LSA origination, flooding, SPF (Dijkstra via petgraph) |
| `src/engine/bgp.rs` | BGP FSM, best-path selection, Adj-RIB/Loc-RIB |
| `src/engine/traffic.rs` | Flow tracing, FIB lookup, link utilization |
| `src/engine/policies.rs` | Route policy DSL parser and application |
| `src/engine/events.rs` | Priority event queue (binary heap) |
| `src/api/topology.rs` | AS, router, interface, link CRUD |
| `src/api/ospf.rs` | OSPF neighbor/route queries |
| `src/api/bgp.rs` | BGP enable, neighbor, route endpoints |
| `src/api/traffic.rs` | Traffic generator/flow endpoints |
| `src/api/simulation.rs` | Start/stop/step, tick-rate, save/load |
| `src/api/websocket.rs` | WebSocket broadcast of tick updates |
| `src/api/schemas.rs` | Request/response DTOs |
| `src/persistence/store.rs` | JSON serialization, sample topology factory |

### Frontend (`frontend/`)
| Path | Purpose |
|------|---------|
| `src/App.tsx` | Root component |
| `src/components/layout/` | AppShell, Toolbar, StatusBar |
| `src/components/topology/TopologyCanvas.tsx` | Cytoscape graph rendering |
| `src/components/panels/` | RouterPanel, LinkPanel detail views |
| `src/store/` | Zustand stores (topology, simulation, selection) |
| `src/api/client.ts` | REST API wrapper functions |
| `src/api/websocket.ts` | WebSocket connection manager |
| `src/types/models.ts` | TypeScript interfaces mirroring backend models |

## Build & Run

### Backend
```bash
cd backend
cargo build           # debug build
cargo run             # starts on http://0.0.0.0:8080
RUST_LOG=info cargo run  # with logging
```

### Frontend
```bash
cd frontend
bun install           # install dependencies
bun run dev           # dev server on http://localhost:3000 (proxies to :8080)
bun run build         # production build → dist/
```

### Full Stack
Start backend first (`cargo run`), then frontend (`bun run dev`). Open http://localhost:3000.

## API Surface

Base path: `/api/v1`

- **Topology:** `/topology/as`, `/topology/routers/{id}`, `/topology/links` — CRUD for AS, routers, interfaces, links
- **OSPF:** `/ospf/{router_id}/lsdb`, `/ospf/{router_id}/neighbors`, `/ospf/{router_id}/routes`
- **BGP:** `/bgp/{router_id}/enable`, `/bgp/{router_id}/neighbors`, `/bgp/{router_id}/routes`
- **Traffic:** `/traffic/generators`, `/traffic/flows`, `/traffic/link-utilization`
- **Simulation:** `/simulation/state`, `/simulation/start`, `/simulation/stop`, `/simulation/step`, `/simulation/tick-rate`, `/simulation/save`, `/simulation/load`, `/simulation/load-sample`
- **Policies:** `/policies` — CRUD for route policies
- **WebSocket:** `/ws` — tick update stream

## Common Development Tasks

### Add an API endpoint
1. Add handler in `backend/src/api/{module}.rs`
2. Register route in that module's `config()` function
3. Add DTO to `schemas.rs` if needed
4. Add client function in `frontend/src/api/client.ts`
5. Wire into the relevant UI component

### Modify simulation logic
1. Edit the relevant engine module (`ospf.rs`, `bgp.rs`, `traffic.rs`, `policies.rs`)
2. Update `models.rs` if data structures change
3. Mirror model changes in `frontend/src/types/models.ts`

### Add a new protocol or feature
1. Create new engine module in `backend/src/engine/`
2. Integrate into `SimulationEngine::step()` in `simulation.rs`
3. Add API endpoints and frontend support

## Testing

No test suite exists yet. Backend supports `cargo test`; frontend can use Vitest.

## Tech Stack Summary

- **Backend:** Rust, Actix-web 4, Tokio, serde, petgraph, parking_lot
- **Frontend:** React 19, TypeScript 5, Vite 8, Zustand 5, Cytoscape 3, Bun
- **Communication:** REST + WebSocket
- **Persistence:** JSON files (no database)

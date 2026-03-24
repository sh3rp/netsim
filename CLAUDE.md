# CLAUDE.md — Instructions for Claude Code

## Project

NetSim: real-time network simulator. Rust backend (Actix-web) + React TypeScript frontend (Vite). Tick-based simulation engine with OSPF, BGP, traffic flows, and route policies.

## Build & Run Commands

```bash
# Backend
cd backend && cargo build
cd backend && RUST_LOG=info cargo run      # serves on :8080

# Frontend
cd frontend && bun install
cd frontend && bun run dev                  # dev server on :3000, proxies to :8080
cd frontend && bun run build                # production build

# Tests (none yet, but these are the commands)
cd backend && cargo test
cd frontend && bun run test
```

## Code Conventions

### Rust (backend)
- Snake_case for variables/functions, PascalCase for types, SCREAMING_SNAKE_CASE for constants
- State shared via `Arc<RwLock<SimulationState>>` passed as Actix `web::Data`
- Serde derives on all model structs for JSON serialization
- Modules organized as: `api/` (handlers), `engine/` (simulation logic), `persistence/` (save/load)
- Handlers return `HttpResponse` with JSON body

### TypeScript (frontend)
- Strict mode enabled
- camelCase for variables/functions, PascalCase for components
- Zustand stores in `src/store/`, one per domain (topology, simulation, selection)
- API calls centralized in `src/api/client.ts`
- Inline `React.CSSProperties` for styling (dark theme, base color `#161b22`)
- Types in `src/types/models.ts` mirror backend `engine/models.rs`

## Architecture Notes

- Backend models are the source of truth; frontend types must stay in sync
- Simulation runs in a background Tokio task, broadcasting tick updates via a `tokio::sync::broadcast` channel to WebSocket subscribers
- OSPF uses petgraph Dijkstra for SPF computation
- BGP best-path selection follows RFC 4271 ordering
- Route policies use a custom DSL parsed in `engine/policies.rs`
- Event queue is a priority-based binary heap in `engine/events.rs`

## Important Patterns

- When modifying data models, update both `backend/src/engine/models.rs` AND `frontend/src/types/models.ts`
- New API endpoints need: handler in `api/`, route registration in module `config()`, client wrapper in `frontend/src/api/client.ts`
- Link state changes trigger OSPF SPF recalculation automatically via the event system
- The frontend proxy config is in `frontend/vite.config.ts` (proxies `/api` and `/ws` to localhost:8080)

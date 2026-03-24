import { create } from "zustand";
import type { TrafficFlow } from "../types/models.ts";

interface SimulationState {
  tick: number;
  running: boolean;
  tickRateMs: number;
  flows: TrafficFlow[];

  setTick: (tick: number) => void;
  setRunning: (running: boolean) => void;
  setTickRate: (ms: number) => void;
  setFlows: (flows: TrafficFlow[]) => void;
}

export const useSimulationStore = create<SimulationState>((set) => ({
  tick: 0,
  running: false,
  tickRateMs: 200,
  flows: [],

  setTick: (tick) => set({ tick }),
  setRunning: (running) => set({ running }),
  setTickRate: (ms) => set({ tickRateMs: ms }),
  setFlows: (flows) => set({ flows }),
}));

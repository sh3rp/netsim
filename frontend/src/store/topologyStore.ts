import { create } from "zustand";
import type {
  AutonomousSystem,
  Link,
  Router,
  TopologySnapshot,
} from "../types/models.ts";

interface TopologyState {
  autonomousSystems: Record<string, AutonomousSystem>;
  links: Record<string, Link>;
  linkUtilization: Record<string, number>;

  setTopology: (snapshot: TopologySnapshot) => void;
  updateLinkUtilization: (util: Record<string, number>) => void;
  clear: () => void;
}

export const useTopologyStore = create<TopologyState>((set) => ({
  autonomousSystems: {},
  links: {},
  linkUtilization: {},

  setTopology: (snapshot) =>
    set({
      autonomousSystems: snapshot.autonomous_systems,
      links: snapshot.links,
    }),

  updateLinkUtilization: (util) =>
    set({ linkUtilization: util }),

  clear: () =>
    set({ autonomousSystems: {}, links: {}, linkUtilization: {} }),
}));

// Derived selectors
export function getAllRouters(
  autonomousSystems: Record<string, AutonomousSystem>,
): Router[] {
  return Object.values(autonomousSystems).flatMap((as_) =>
    Object.values(as_.routers),
  );
}

import { create } from "zustand";
import type {
  AutonomousSystem,
  Link,
  Router,
  TopologySnapshot,
} from "../types/models.ts";

interface TopologyState {
  autonomousSystems: Record<string, AutonomousSystem>;
  standaloneRouters: Record<string, Router>;
  links: Record<string, Link>;
  linkUtilization: Record<string, number>;

  setTopology: (snapshot: TopologySnapshot) => void;
  updateLinkUtilization: (util: Record<string, number>) => void;
  clear: () => void;
}

export const useTopologyStore = create<TopologyState>((set) => ({
  autonomousSystems: {},
  standaloneRouters: {},
  links: {},
  linkUtilization: {},

  setTopology: (snapshot) =>
    set({
      autonomousSystems: snapshot.autonomous_systems,
      standaloneRouters: snapshot.standalone_routers ?? {},
      links: snapshot.links,
    }),

  updateLinkUtilization: (util) =>
    set({ linkUtilization: util }),

  clear: () =>
    set({ autonomousSystems: {}, standaloneRouters: {}, links: {}, linkUtilization: {} }),
}));

// Derived selectors
export function getAllRouters(
  autonomousSystems: Record<string, AutonomousSystem>,
  standaloneRouters?: Record<string, Router>,
): Router[] {
  const asRouters = Object.values(autonomousSystems).flatMap((as_) =>
    Object.values(as_.routers),
  );
  const standalone = standaloneRouters
    ? Object.values(standaloneRouters)
    : [];
  return [...standalone, ...asRouters];
}

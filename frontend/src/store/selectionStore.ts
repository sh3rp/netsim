import { create } from "zustand";

interface SelectionState {
  selectedNodeId: string | null;
  selectedNodeType: "router" | "as" | "link" | null;
  selectedAsId: string | null;

  selectRouter: (routerId: string, asId: string) => void;
  selectAS: (asId: string) => void;
  selectLink: (linkId: string) => void;
  clearSelection: () => void;
}

export const useSelectionStore = create<SelectionState>((set) => ({
  selectedNodeId: null,
  selectedNodeType: null,
  selectedAsId: null,

  selectRouter: (routerId, asId) =>
    set({
      selectedNodeId: routerId,
      selectedNodeType: "router",
      selectedAsId: asId,
    }),

  selectAS: (asId) =>
    set({
      selectedNodeId: asId,
      selectedNodeType: "as",
      selectedAsId: asId,
    }),

  selectLink: (linkId) =>
    set({
      selectedNodeId: linkId,
      selectedNodeType: "link",
      selectedAsId: null,
    }),

  clearSelection: () =>
    set({
      selectedNodeId: null,
      selectedNodeType: null,
      selectedAsId: null,
    }),
}));

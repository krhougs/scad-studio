// Zustand store for UI shell state only. No protocol business state is allowed
// here; those fields live inside the wasm client snapshot.
//
// Phase 6: `openTabs` holds DocumentTab descriptors (id/label/path/kind) only.
// Never store document contents (markdown source, image bytes, scad text) in
// this store — contents are loaded on demand inside viewer components.

import { create } from "zustand";

export type DocumentTabKind = "markdown" | "image" | "scad" | "mesh";

export type DocumentTab = {
  id: string;
  label: string;
  path: unknown;
  kind: DocumentTabKind;
};

export type UiState = {
  route: string;
  openTabs: DocumentTab[];
  activeTabId: string | null;
  activeRail: string;
  sidePanelOpen: boolean;
  isSettingsModalOpen: boolean;
  inputDraft: string;
};

export type UiActions = {
  setRoute: (route: string) => void;
  openTab: (tab: DocumentTab) => void;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  setActiveRail: (id: string) => void;
  toggleSidePanel: () => void;
  setSettingsModalOpen: (value: boolean) => void;
  setInputDraft: (value: string) => void;
};

export type UiStore = UiState & UiActions;

export const useUiStore = create<UiStore>((set) => ({
  route: "/",
  openTabs: [],
  activeTabId: null,
  activeRail: "chat",
  sidePanelOpen: true,
  isSettingsModalOpen: false,
  inputDraft: "",
  setRoute: (route) => set({ route }),
  openTab: (tab) =>
    set((prev) => {
      const existing = prev.openTabs.find((t) => t.id === tab.id);
      if (existing) {
        return { activeTabId: tab.id };
      }
      return {
        openTabs: [...prev.openTabs, tab],
        activeTabId: tab.id,
      };
    }),
  closeTab: (id) =>
    set((prev) => {
      const nextTabs = prev.openTabs.filter((tab) => tab.id !== id);
      const nextActive =
        prev.activeTabId === id
          ? (nextTabs[nextTabs.length - 1]?.id ?? null)
          : prev.activeTabId;
      return { openTabs: nextTabs, activeTabId: nextActive };
    }),
  setActiveTab: (id) => set({ activeTabId: id }),
  setActiveRail: (id) => set({ activeRail: id }),
  toggleSidePanel: () => set((prev) => ({ sidePanelOpen: !prev.sidePanelOpen })),
  setSettingsModalOpen: (value) => set({ isSettingsModalOpen: value }),
  setInputDraft: (value) => set({ inputDraft: value }),
}));

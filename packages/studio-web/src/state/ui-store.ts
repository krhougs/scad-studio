// Zustand store for UI shell state only. Protocol business state lives in
// protocol-store.ts, not here.
//
// Phase 6: `openTabs` holds DocumentTab descriptors (id/label/path/kind) only.
// Never store document contents (markdown source, image bytes, scad text) in
// this store — contents are loaded on demand inside viewer or workbench logic.

import { create } from "zustand";
import type { LeftPanelId } from "../workbench/left-panel-routing";

export type DocumentTabKind =
  | "markdown"
  | "image"
  | "scad"
  | "mesh"
  | "cadquery";

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
  activeRail: LeftPanelId;
  sidePanelOpen: boolean;
  isSettingsModalOpen: boolean;
};

export type UiActions = {
  setRoute: (route: string) => void;
  openTab: (tab: DocumentTab) => void;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  setActiveRail: (id: LeftPanelId) => void;
  toggleSidePanel: () => void;
  setSettingsModalOpen: (value: boolean) => void;
};

export type UiStore = UiState & UiActions;

export const useUiStore = create<UiStore>((set) => ({
  route: "/",
  openTabs: [],
  activeTabId: null,
  activeRail: "chat",
  sidePanelOpen: true,
  isSettingsModalOpen: false,
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
  toggleSidePanel: () =>
    set((prev) => ({ sidePanelOpen: !prev.sidePanelOpen })),
  setSettingsModalOpen: (value) => set({ isSettingsModalOpen: value }),
}));

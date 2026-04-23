// Zustand store for UI shell state only. No protocol business state is allowed
// here; those fields live inside the wasm client snapshot.

import { create } from "zustand";

export type UiState = {
  route: string;
  openTabs: string[];
  activeTab: string | null;
  sidePanelOpen: boolean;
  isSettingsModalOpen: boolean;
  inputDraft: string;
};

export type UiActions = {
  setRoute: (route: string) => void;
  openTab: (tabId: string) => void;
  closeTab: (tabId: string) => void;
  focusTab: (tabId: string) => void;
  toggleSidePanel: () => void;
  setSettingsModalOpen: (value: boolean) => void;
  setInputDraft: (value: string) => void;
};

export type UiStore = UiState & UiActions;

export const useUiStore = create<UiStore>((set) => ({
  route: "/",
  openTabs: [],
  activeTab: null,
  sidePanelOpen: true,
  isSettingsModalOpen: false,
  inputDraft: "",
  setRoute: (route) => set({ route }),
  openTab: (tabId) =>
    set((prev) => {
      if (prev.openTabs.includes(tabId)) {
        return { activeTab: tabId };
      }
      return {
        openTabs: [...prev.openTabs, tabId],
        activeTab: tabId,
      };
    }),
  closeTab: (tabId) =>
    set((prev) => {
      const nextTabs = prev.openTabs.filter((id) => id !== tabId);
      const nextActive =
        prev.activeTab === tabId
          ? (nextTabs[nextTabs.length - 1] ?? null)
          : prev.activeTab;
      return { openTabs: nextTabs, activeTab: nextActive };
    }),
  focusTab: (tabId) => set({ activeTab: tabId }),
  toggleSidePanel: () => set((prev) => ({ sidePanelOpen: !prev.sidePanelOpen })),
  setSettingsModalOpen: (value) => set({ isSettingsModalOpen: value }),
  setInputDraft: (value) => set({ inputDraft: value }),
}));

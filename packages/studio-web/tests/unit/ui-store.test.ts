// Unit tests for the Zustand UI store — DocumentTab open/close/activate flow.

import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore, type DocumentTab } from "../../src/state/ui-store";

const tabA: DocumentTab = {
  id: "a",
  label: "a.md",
  path: { path_segments: ["a.md"] },
  kind: "markdown",
};
const tabB: DocumentTab = {
  id: "b",
  label: "b.png",
  path: { path_segments: ["b.png"] },
  kind: "image",
};
const tabC: DocumentTab = {
  id: "c",
  label: "c.scad",
  path: { path_segments: ["c.scad"] },
  kind: "scad",
};

describe("UI store document tabs", () => {
  beforeEach(() => {
    useUiStore.setState({ openTabs: [], activeTabId: null });
  });

  it("opens 3 tabs and keeps the last one active", () => {
    useUiStore.getState().openTab(tabA);
    useUiStore.getState().openTab(tabB);
    useUiStore.getState().openTab(tabC);
    const state = useUiStore.getState();
    expect(state.openTabs.map((t) => t.id)).toEqual(["a", "b", "c"]);
    expect(state.activeTabId).toBe("c");
  });

  it("closing the middle tab preserves the active tab", () => {
    useUiStore.getState().openTab(tabA);
    useUiStore.getState().openTab(tabB);
    useUiStore.getState().openTab(tabC);
    useUiStore.getState().closeTab("b");
    const state = useUiStore.getState();
    expect(state.openTabs.map((t) => t.id)).toEqual(["a", "c"]);
    expect(state.activeTabId).toBe("c");
  });

  it("closing the active tab falls back to the last remaining", () => {
    useUiStore.getState().openTab(tabA);
    useUiStore.getState().openTab(tabB);
    useUiStore.getState().closeTab("b");
    expect(useUiStore.getState().activeTabId).toBe("a");
  });

  it("closing the only tab clears activeTabId", () => {
    useUiStore.getState().openTab(tabA);
    useUiStore.getState().closeTab("a");
    const state = useUiStore.getState();
    expect(state.openTabs).toEqual([]);
    expect(state.activeTabId).toBeNull();
  });

  it("opening a tab that already exists just re-focuses it", () => {
    useUiStore.getState().openTab(tabA);
    useUiStore.getState().openTab(tabB);
    useUiStore.getState().setActiveTab("a");
    useUiStore.getState().openTab(tabB);
    const state = useUiStore.getState();
    expect(state.openTabs.map((t) => t.id)).toEqual(["a", "b"]);
    expect(state.activeTabId).toBe("b");
  });
});

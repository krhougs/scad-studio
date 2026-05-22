import { describe, expect, it } from "vitest";
import {
  shouldRefreshDocumentForWatch,
  shouldRefreshScadSettingsForWatch,
} from "../../src/workbench/watch-refresh";

describe("shouldRefreshDocumentForWatch", () => {
  it("refreshes CadQuery only when the watched path matches the active tab", () => {
    const tab = {
      kind: "cadquery" as const,
      path: { workspace_id: "ws", path_segments: ["parts", "model.py"] },
    };
    expect(
      shouldRefreshDocumentForWatch(tab, new Set(["parts/model.py"]), false),
    ).toBe(true);
    expect(shouldRefreshDocumentForWatch(tab, new Set(), false)).toBe(false);
    expect(
      shouldRefreshDocumentForWatch(tab, new Set(["outputs/model.step"]), false),
    ).toBe(false);
  });

  it("keeps directory fallback for passive file viewers", () => {
    const tab = {
      kind: "markdown" as const,
      path: { workspace_id: "ws", path_segments: ["docs", "note.md"] },
    };
    expect(shouldRefreshDocumentForWatch(tab, new Set(), false)).toBe(true);
    expect(shouldRefreshDocumentForWatch(tab, new Set(), true)).toBe(false);
  });

  it("refreshes Scad preview on directory watch events but not settings-only events", () => {
    const tab = {
      kind: "scad" as const,
      path: { workspace_id: "ws", path_segments: ["examples", "cube.scad"] },
    };
    expect(shouldRefreshDocumentForWatch(tab, new Set(), false)).toBe(true);
    expect(
      shouldRefreshDocumentForWatch(tab, new Set(["examples"]), false),
    ).toBe(false);
    expect(shouldRefreshDocumentForWatch(tab, new Set(), true)).toBe(false);
  });

  it("refreshes Scad settings on settings matches and directory watch events", () => {
    const tab = { kind: "scad" as const };
    expect(shouldRefreshScadSettingsForWatch(tab, new Set(), false)).toBe(true);
    expect(
      shouldRefreshScadSettingsForWatch(
        tab,
        new Set(["examples/cube.scad.json"]),
        true,
      ),
    ).toBe(true);
    expect(
      shouldRefreshScadSettingsForWatch(
        tab,
        new Set(["examples/cube.scad"]),
        false,
      ),
    ).toBe(false);
  });
});

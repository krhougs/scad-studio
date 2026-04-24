import { describe, expect, it } from "vitest";
import {
  configuredSlicerRecords,
  decodeConfigLoad,
  describeConfigGaps,
  normalizeAppConfig,
} from "../../src/config/app-config";

describe("app-config", () => {
  it("decodeConfigLoad normalizes persisted config", () => {
    const decoded = decodeConfigLoad({
      payload: {
        json: JSON.stringify({
          openscad_path: " /usr/bin/openscad ",
          slicers: [
            { name: " slicer ", path: " /usr/bin/slicer " },
            { name: "", path: "" },
          ],
        }),
      },
    });
    expect(decoded.config).toEqual({
      openscad_path: "/usr/bin/openscad",
      slicers: [
        { name: "slicer", path: "/usr/bin/slicer" },
        { name: "", path: "" },
      ],
      recent_workspaces: [],
      floating_panel_opacity: 0.85,
    });
  });

  it("configuredSlicerRecords filters incomplete rows", () => {
    expect(
      configuredSlicerRecords({
        slicers: [
          { name: "a", path: "/tmp/a" },
          { name: "missing-path", path: "" },
        ],
      }),
    ).toEqual([{ name: "a", path: "/tmp/a" }]);
  });

  it("describeConfigGaps reports missing openscad path and slicers", () => {
    expect(describeConfigGaps(normalizeAppConfig({}))).toEqual([
      "openscad path missing",
      "no slicer configured",
    ]);
  });
});

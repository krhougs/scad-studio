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
      left_panel_width: 360,
      right_panel_width: 320,
      display_unit: "millimeter",
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

  it("normalizes persisted panel widths and display units", () => {
    expect(
      normalizeAppConfig({
        left_panel_width: 120,
        right_panel_width: 900,
        display_unit: "inch",
      }),
    ).toMatchObject({
      left_panel_width: 280,
      right_panel_width: 640,
      display_unit: "inch",
    });
    expect(normalizeAppConfig({ display_unit: "feet" as never })).toMatchObject({
      display_unit: "millimeter",
    });
  });
});

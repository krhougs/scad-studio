import { describe, expect, it } from "vitest";
import {
  configuredSlicerRecords,
  decodeConfigLoad,
  describeConfigGaps,
  normalizeAppConfig,
  toConfigSaveRequest,
} from "../../src/config/app-config";

describe("app-config", () => {
  it("decodeConfigLoad normalizes persisted config", () => {
    const decoded = decodeConfigLoad({
      type: "config_loaded",
      payload: {
        config: {
          openscad_path: " /usr/bin/openscad ",
          slicers: [
            { name: " slicer ", path: " /usr/bin/slicer " },
            { name: "", path: "" },
          ],
          recent_workspaces: [],
          floating_panel_opacity: 0.85,
          left_panel_width: 360,
          right_panel_width: 320,
          display_unit: "millimeter",
          camera_overlay_pos: null,
          camera_overlay_size: null,
          param_panel_pos: null,
          param_panel_size: null,
          log_panel_pos: null,
          log_panel_size: null,
        },
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
      camera_overlay_pos: null,
      camera_overlay_size: null,
      param_panel_pos: null,
      param_panel_size: null,
      log_panel_pos: null,
      log_panel_size: null,
    });
  });

  it("toConfigSaveRequest returns typed protocol DTO without json payload", () => {
    expect(
      toConfigSaveRequest({
        openscad_path: " /usr/bin/openscad ",
        slicers: [{ name: " slicer ", path: " /usr/bin/slicer " }],
        recent_workspaces: ["/tmp/ws", 1 as never],
        camera_overlay_pos: [10, 20],
        camera_overlay_size: [Number.POSITIVE_INFINITY, 120],
      }),
    ).toEqual({
      config: {
        openscad_path: "/usr/bin/openscad",
        slicers: [{ name: "slicer", path: "/usr/bin/slicer" }],
        recent_workspaces: ["/tmp/ws"],
        floating_panel_opacity: 0.85,
        left_panel_width: 360,
        right_panel_width: 320,
        display_unit: "millimeter",
        camera_overlay_pos: [10, 20],
        camera_overlay_size: null,
        param_panel_pos: null,
        param_panel_size: null,
        log_panel_pos: null,
        log_panel_size: null,
      },
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
        floating_panel_opacity: 0.41999998688697815,
        left_panel_width: 120,
        right_panel_width: 900,
        display_unit: "inch",
      }),
    ).toMatchObject({
      floating_panel_opacity: 0.42,
      left_panel_width: 280,
      right_panel_width: 640,
      display_unit: "inch",
    });
    expect(normalizeAppConfig({ display_unit: "feet" as never })).toMatchObject({
      display_unit: "millimeter",
    });
  });
});

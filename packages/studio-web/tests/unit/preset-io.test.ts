import { describe, expect, it } from "vitest";
import {
  deriveLegacyPresetPath,
  deriveLegacyPresetPaths,
  derivePresetPath,
  derivePresetPathLabel,
  parsePresetFile,
  stringifyPresetFile,
} from "../../src/workbench/preset-io";

describe("preset-io", () => {
  it("derivePresetPath switches to desktop-compatible .scad.json", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    const next = derivePresetPath(path) as { path_segments: string[] };
    expect(next.path_segments).toEqual(["examples", "params-cube.scad.json"]);
  });

  it("derivePresetPathLabel returns a joined preview", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    expect(derivePresetPathLabel(path)).toBe("examples/params-cube.scad.json");
  });

  it("deriveLegacyPresetPath keeps compatibility with stem.presets.json", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    const next = deriveLegacyPresetPath(path) as { path_segments: string[] };
    expect(next.path_segments).toEqual(["examples", "params-cube.presets.json"]);
  });

  it("deriveLegacyPresetPaths includes old web and stem candidates", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    const paths = deriveLegacyPresetPaths(path) as Array<{ path_segments: string[] }>;
    expect(paths.map((item) => item.path_segments)).toEqual([
      ["examples", "params-cube.scad.presets.json"],
      ["examples", "params-cube.presets.json"],
    ]);
  });

  it("parsePresetFile rejects unsupported legacy versions", () => {
    expect(() => parsePresetFile(JSON.stringify({ version: 2 }))).toThrow();
  });

  it("parsePresetFile returns parsed presets from legacy web shape", () => {
    const text = JSON.stringify({
      version: 1,
      presets: [{ name: "big", defines: ["size=20"] }],
    });
    const file = parsePresetFile(text);
    expect(file.presets[0].name).toBe("big");
    expect(file.presets[0].values).toEqual({ size: 20 });
  });

  it("parsePresetFile accepts desktop-compatible shared shape", () => {
    const text = JSON.stringify({
      presets: {
        desktop: {
          size: 20,
          wall: 4,
          enabled: true,
          flavor: "draft",
        },
      },
    });
    const file = parsePresetFile(text);
    expect(file.presets).toEqual([
      {
        name: "desktop",
        values: {
          enabled: true,
          flavor: "draft",
          size: 20,
          wall: 4,
        },
      },
    ]);
  });

  it("parsePresetFile returns default preview appearance when shared file omits it", () => {
    const text = JSON.stringify({ presets: {} });
    const file = parsePresetFile(text) as unknown as {
      previewAppearance?: {
        backgroundColor: string;
        gridMajorColor: string;
        gridMinorColor: string;
        lightingIntensity: number;
      };
    };

    expect(file.previewAppearance).toEqual({
      backgroundColor: "#181b20",
      gridMajorColor: "#5a6573",
      gridMinorColor: "#343b45",
      lightingIntensity: 1.25,
    });
  });

  it("parsePresetFile accepts preview appearance beside presets", () => {
    const text = JSON.stringify({
      presets: {
        bright: {
          size: 20,
        },
      },
      previewAppearance: {
        backgroundColor: "#20242b",
        gridMajorColor: "#7c8795",
        gridMinorColor: "#46505d",
        lightingIntensity: 1.6,
      },
    });
    const file = parsePresetFile(text) as unknown as {
      previewAppearance?: {
        backgroundColor: string;
        gridMajorColor: string;
        gridMinorColor: string;
        lightingIntensity: number;
      };
    };

    expect(file.previewAppearance).toEqual({
      backgroundColor: "#20242b",
      gridMajorColor: "#7c8795",
      gridMinorColor: "#46505d",
      lightingIntensity: 1.6,
    });
  });

  it("parsePresetFile normalizes invalid preview appearance values", () => {
    const text = JSON.stringify({
      presets: {},
      previewAppearance: {
        backgroundColor: "red",
        gridMajorColor: "#bad",
        gridMinorColor: "transparent",
        lightingIntensity: 100,
      },
    });
    const file = parsePresetFile(text) as unknown as {
      previewAppearance?: {
        backgroundColor: string;
        gridMajorColor: string;
        gridMinorColor: string;
        lightingIntensity: number;
      };
    };

    expect(file.previewAppearance).toEqual({
      backgroundColor: "#181b20",
      gridMajorColor: "#5a6573",
      gridMinorColor: "#343b45",
      lightingIntensity: 3,
    });
  });

  it("stringifyPresetFile round-trips", () => {
    const file = {
      presets: [{ name: "a", values: { flag: true, mode: "fast", x: 1 } }],
    };
    const text = stringifyPresetFile(file);
    const back = parsePresetFile(text);
    expect(back).toEqual(file);
    expect(JSON.parse(text)).toEqual({
      presets: {
        a: {
          flag: true,
          mode: "fast",
          x: 1,
        },
      },
    });
  });

  it("stringifyPresetFile writes presets and preview appearance together", () => {
    const file = {
      presets: [{ name: "a", values: { flag: true, mode: "fast", x: 1 } }],
      previewAppearance: {
        backgroundColor: "#20242b",
        gridMajorColor: "#7c8795",
        gridMinorColor: "#46505d",
        lightingIntensity: 1.6,
      },
    };
    const text = stringifyPresetFile(file);

    expect(JSON.parse(text)).toEqual({
      presets: {
        a: {
          flag: true,
          mode: "fast",
          x: 1,
        },
      },
      previewAppearance: {
        backgroundColor: "#20242b",
        gridMajorColor: "#7c8795",
        gridMinorColor: "#46505d",
        lightingIntensity: 1.6,
      },
    });
    expect(parsePresetFile(text)).toEqual(file);
  });
});

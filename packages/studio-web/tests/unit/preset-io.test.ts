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
});

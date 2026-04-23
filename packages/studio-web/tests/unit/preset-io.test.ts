import { describe, expect, it } from "vitest";
import {
  derivePresetPath,
  derivePresetPathLabel,
  parsePresetFile,
  stringifyPresetFile,
} from "../../src/workbench/preset-io";

describe("preset-io", () => {
  it("derivePresetPath appends .presets.json to the last segment", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    const next = derivePresetPath(path) as { path_segments: string[] };
    expect(next.path_segments).toEqual([
      "examples",
      "params-cube.scad.presets.json",
    ]);
  });

  it("derivePresetPathLabel returns a joined preview", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["examples", "params-cube.scad"],
    };
    expect(derivePresetPathLabel(path)).toBe(
      "examples/params-cube.scad.presets.json",
    );
  });

  it("parsePresetFile rejects unsupported versions", () => {
    expect(() => parsePresetFile(JSON.stringify({ version: 2 }))).toThrow();
  });

  it("parsePresetFile returns parsed presets", () => {
    const text = JSON.stringify({
      version: 1,
      presets: [{ name: "big", defines: ["size=20"] }],
    });
    const file = parsePresetFile(text);
    expect(file.presets[0].name).toBe("big");
    expect(file.presets[0].defines).toEqual(["size=20"]);
  });

  it("stringifyPresetFile round-trips", () => {
    const file = {
      version: 1 as const,
      presets: [{ name: "a", defines: ["x=1"] }],
    };
    const text = stringifyPresetFile(file);
    const back = parsePresetFile(text);
    expect(back).toEqual(file);
  });
});

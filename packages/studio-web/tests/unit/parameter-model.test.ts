import { describe, expect, it } from "vitest";
import {
  applyPresetValues,
  currentParameterValues,
  formatCurrentDefines,
  mergeParameterEntries,
  parseParameterSource,
  restoreParameterValue,
  sliderBounds,
  updateParameterValue,
} from "../../src/workbench/parameter-model";

describe("parameter-model", () => {
  it("parses Customizer parameter types from source", () => {
    const parsed = parseParameterSource(
      'size = 10; // [5:1:30]\nenabled = true;\nmode = "draft"; // ["draft", "fine"]\n',
    );

    expect(parsed.entries.map((entry) => entry.definition.name)).toEqual([
      "size",
      "enabled",
      "mode",
    ]);
    expect(parsed.entries.map((entry) => entry.value)).toEqual([
      10,
      true,
      "draft",
    ]);
  });

  it("merges reparsed definitions while preserving current values", () => {
    const first = parseParameterSource("size = 10;\nwall = 2;\n").entries;
    const changed = updateParameterValue(first, "size", 24);
    const next = parseParameterSource("size = 10;\nmode = \"draft\"; // [draft, fine]\n").entries;

    expect(mergeParameterEntries(changed, next).map((entry) => entry.value)).toEqual([
      24,
      "draft",
    ]);
  });

  it("formats current entries with shared define semantics", () => {
    const parsed = parseParameterSource(
      'size = 10;\nenabled = true;\nmode = "draft"; // [draft, fine]\n',
    ).entries;
    const changed = applyPresetValues(parsed, { size: 20, mode: "fine" });

    expect(formatCurrentDefines(changed)).toEqual([
      "size=20",
      "enabled=true",
      'mode="fine"',
    ]);
    expect(currentParameterValues(changed)).toEqual({
      size: 20,
      enabled: true,
      mode: "fine",
    });
  });

  it("restores one parameter without touching siblings", () => {
    const parsed = parseParameterSource("size = 10;\nwall = 2;\n").entries;
    const changed = updateParameterValue(updateParameterValue(parsed, "size", 24), "wall", 4);

    expect(restoreParameterValue(changed, "size").map((entry) => entry.value)).toEqual([
      10,
      4,
    ]);
  });

  it("derives slider bounds with explicit range and negative fallback range", () => {
    const ranged = parseParameterSource("size = 10; // [5:0.5:30]\n").entries[0];
    expect(sliderBounds(ranged)).toEqual({
      min: 5,
      max: 30,
      step: 0.5,
    });

    const inferred = updateParameterValue(
      parseParameterSource("offset = -3;\n").entries,
      "offset",
      -8,
    )[0];
    expect(sliderBounds(inferred)).toEqual({
      min: -6,
      max: 6,
      step: 1,
    });
  });
});

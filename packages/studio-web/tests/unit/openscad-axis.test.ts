import { describe, expect, it } from "vitest";
import {
  OPENSCAD_AXIS_DIRECTIONS,
  viewerDirectionForOpenScadAxis,
} from "../../src/viewers/openscad-axis";

describe("openscad-axis", () => {
  it("maps OpenSCAD semantic axes to viewer-space directions", () => {
    expect(viewerDirectionForOpenScadAxis("x")).toEqual([1, 0, 0]);
    expect(viewerDirectionForOpenScadAxis("y")).toEqual([0, 0, -1]);
    expect(viewerDirectionForOpenScadAxis("z")).toEqual([0, 1, 0]);
  });

  it("keeps the public axis table ordered as X, Y, Z", () => {
    expect(OPENSCAD_AXIS_DIRECTIONS.map((axis: { id: string }) => axis.id)).toEqual([
      "x",
      "y",
      "z",
    ]);
  });
});

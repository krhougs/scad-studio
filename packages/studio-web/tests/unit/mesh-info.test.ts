import { describe, expect, it } from "vitest";
import { computeMeshInfo } from "../../src/viewers/mesh-info";

describe("mesh-info", () => {
  it("computes bounds, center, dimensions and radius from positions", () => {
    const info = computeMeshInfo(
      new Float32Array([
        -2, -1, 0,
        4, 3, 10,
        1, 2, -2,
      ]),
      new Uint32Array([0, 1, 2]),
    );

    expect(info).toMatchObject({
      vertices: 3,
      indices: 3,
      bounds: {
        min: [-2, -1, -2],
        max: [4, 3, 10],
      },
      center: [1, 1, 4],
      dimensions: [6, 4, 12],
    });
    expect(info?.radius).toBeCloseTo(Math.sqrt(196) / 2, 6);
  });

  it("returns null for empty or invalid positions", () => {
    expect(computeMeshInfo(new Float32Array(), null)).toBeNull();
    expect(computeMeshInfo(new Float32Array([1, 2]), null)).toBeNull();
  });
});

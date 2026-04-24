import { describe, expect, it } from "vitest";
import {
  applyPreset,
  classifyPointerMode,
  distanceTo,
  fitCameraToBounds,
  orbitBy,
  panBy,
  resetCamera,
  sphericalFromCamera,
  updateCameraFromSpherical,
  zoomBy,
} from "../../src/canvas/camera-controls";
import {
  defaultCameraState,
  PRESET_STATES,
} from "../../src/canvas/camera-state";

describe("camera-controls", () => {
  it("applyPreset mirrors the preset table", () => {
    const next = applyPreset("top");
    expect(next.position).toEqual(PRESET_STATES.top.position);
    expect(next.target).toEqual([0, 0, 0]);
    expect(next.up).toEqual([0, 1, 0]);
  });

  it("reset returns the default iso state", () => {
    const base = resetCamera();
    expect(base.position).toEqual(defaultCameraState().position);
  });

  it("zoomBy shrinks and grows the distance", () => {
    const base = applyPreset("iso");
    const before = distanceTo(base);
    const closer = zoomBy(base, -0.2);
    expect(distanceTo(closer)).toBeLessThan(before);
    const farther = zoomBy(base, 0.2);
    expect(distanceTo(farther)).toBeGreaterThan(before);
  });

  it("orbitBy preserves distance and rotates position", () => {
    const base = applyPreset("front");
    const baseDist = distanceTo(base);
    const rotated = orbitBy(base, Math.PI / 2, 0);
    expect(Math.abs(distanceTo(rotated) - baseDist)).toBeLessThan(1e-6);
    expect(rotated.position[0]).not.toBeCloseTo(base.position[0], 3);
    expect(rotated.position[1]).not.toBeCloseTo(base.position[1], 3);
  });

  it("panBy shifts position and target together", () => {
    const base = applyPreset("iso");
    const shifted = panBy(base, 2, 3);
    expect(shifted.position[0]).not.toEqual(base.position[0]);
    expect(shifted.target[0]).not.toEqual(base.target[0]);
    const dx = shifted.position[0] - base.position[0];
    const dy = shifted.target[0] - base.target[0];
    expect(Math.abs(dx - dy)).toBeLessThan(1e-6);
  });

  it("classifyPointerMode maps buttons + alt", () => {
    expect(classifyPointerMode({ button: 0, altKey: false })).toBe("orbit");
    expect(classifyPointerMode({ button: 0, altKey: true })).toBe("pan");
    expect(classifyPointerMode({ button: 2, altKey: false })).toBe("pan");
    expect(classifyPointerMode({ button: 1, altKey: false })).toBe("none");
  });

  it("fits camera distance to model bounds and aspect ratio", () => {
    const small = fitCameraToBounds(
      { min: [-5, -5, -5], max: [5, 5, 5] },
      "iso",
      16 / 9,
    );
    const large = fitCameraToBounds(
      { min: [-100, -50, -25], max: [100, 50, 25] },
      "iso",
      16 / 9,
    );

    expect(distanceTo(large)).toBeGreaterThan(distanceTo(small));
    expect(small.target).toEqual([0, 0, 0]);
    expect(large.target).toEqual([0, 0, 0]);
  });

  it("updates camera from target, distance, azimuth and elevation", () => {
    const base = fitCameraToBounds(
      { min: [10, 0, -10], max: [30, 20, 10] },
      "front",
      1,
    );
    const spherical = sphericalFromCamera(base);
    const moved = updateCameraFromSpherical(base, {
      target: [20, 10, 0],
      distance: spherical.distance * 2,
      azimuthDeg: 90,
      elevationDeg: 30,
    });

    expect(moved.target).toEqual([20, 10, 0]);
    expect(distanceTo(moved)).toBeCloseTo(spherical.distance * 2, 5);
    expect(sphericalFromCamera(moved).azimuthDeg).toBeCloseTo(90, 5);
    expect(sphericalFromCamera(moved).elevationDeg).toBeCloseTo(30, 5);
  });

  it("ignores non-finite spherical patches", () => {
    const base = fitCameraToBounds(
      { min: [-10, -10, -10], max: [10, 10, 10] },
      "front",
      1,
    );
    const unchanged = updateCameraFromSpherical(base, {
      target: [Number.NaN, 2, 3],
      distance: Number.NaN,
      azimuthDeg: Number.NaN,
      elevationDeg: Number.NaN,
    });

    expect(unchanged.target).toEqual(base.target);
    expect(distanceTo(unchanged)).toBeCloseTo(distanceTo(base), 5);
    expect(sphericalFromCamera(unchanged).azimuthDeg).toBeCloseTo(
      sphericalFromCamera(base).azimuthDeg,
      5,
    );
    expect(sphericalFromCamera(unchanged).elevationDeg).toBeCloseTo(
      sphericalFromCamera(base).elevationDeg,
      5,
    );
  });
});

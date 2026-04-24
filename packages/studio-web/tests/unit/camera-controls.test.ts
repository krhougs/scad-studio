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
  it("defines six project-coordinate orthographic camera presets", () => {
    const cases = [
      { preset: "top" as const, position: [0, 0, 50], up: [0, 1, 0] },
      { preset: "bottom" as const, position: [0, 0, -50], up: [0, -1, 0] },
      { preset: "front" as const, position: [0, -50, 0], up: [0, 0, 1] },
      { preset: "back" as const, position: [0, 50, 0], up: [0, 0, 1] },
      { preset: "right" as const, position: [50, 0, 0], up: [0, 0, 1] },
      { preset: "left" as const, position: [-50, 0, 0], up: [0, 0, 1] },
    ];

    for (const item of cases) {
      const camera = applyPreset(item.preset);
      expect(camera.position).toEqual(item.position);
      expect(camera.up).toEqual(item.up);
    }
  });

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

  it("zoomBy follows desktop OrbitControls wheel direction", () => {
    const base = applyPreset("iso");
    const before = distanceTo(base);
    const closer = zoomBy(base, 1);
    expect(distanceTo(closer)).toBeLessThan(before);
    const farther = zoomBy(base, -1);
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

  it("orbitBy allows crossing over the top of the model", () => {
    const base = applyPreset("front");
    const moved = orbitBy(base, 0, Math.PI * 1.25);
    expect(moved.position[2]).toBeLessThan(base.target[2]);
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
    expect(classifyPointerMode({ button: 1, altKey: false })).toBe("pan");
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

  it("fits camera presets without changing project-coordinate view directions", () => {
    const bounds = {
      min: [10, -20, -30] as [number, number, number],
      max: [30, 20, 30] as [number, number, number],
    };
    const center = [20, 0, 0];
    const directions = [
      { preset: "top" as const, sign: [0, 0, 1], up: [0, 1, 0] },
      { preset: "bottom" as const, sign: [0, 0, -1], up: [0, -1, 0] },
      { preset: "front" as const, sign: [0, -1, 0], up: [0, 0, 1] },
      { preset: "back" as const, sign: [0, 1, 0], up: [0, 0, 1] },
      { preset: "right" as const, sign: [1, 0, 0], up: [0, 0, 1] },
      { preset: "left" as const, sign: [-1, 0, 0], up: [0, 0, 1] },
    ];

    for (const item of directions) {
      const camera = fitCameraToBounds(bounds, item.preset, 1);
      const distance = distanceTo(camera);
      expect(camera.target).toEqual(center);
      expect(camera.up).toEqual(item.up);
      expect(camera.position[0] - camera.target[0]).toBeCloseTo(
        item.sign[0] * distance,
        5,
      );
      expect(camera.position[1] - camera.target[1]).toBeCloseTo(
        item.sign[1] * distance,
        5,
      );
      expect(camera.position[2] - camera.target[2]).toBeCloseTo(
        item.sign[2] * distance,
        5,
      );
    }
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

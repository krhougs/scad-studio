import { describe, expect, it } from "vitest";
import {
  applyPreset,
  classifyPointerMode,
  distanceTo,
  orbitBy,
  panBy,
  resetCamera,
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
});

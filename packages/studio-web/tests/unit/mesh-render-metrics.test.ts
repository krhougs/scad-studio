import { describe, expect, it } from "vitest";
import {
  distanceTo,
  fitCameraToBounds,
  updateCameraFromSpherical,
} from "../../src/canvas/camera-controls";
import {
  clippingPlanesForBounds,
  meshRenderInputsReady,
  meshSceneMetrics,
} from "../../src/viewers/mesh-render-metrics";
import type { MeshInfo } from "../../src/viewers/mesh-info";

const INFO: MeshInfo = {
  vertices: 8,
  indices: 36,
  bounds: { min: [-100, -50, -25], max: [100, 50, 25] },
  center: [0, 0, 0],
  dimensions: [200, 100, 50],
  radius: 114.564392373896,
};

const SMALL_INFO: MeshInfo = {
  vertices: 8,
  indices: 36,
  bounds: { min: [-5, -5, -5], max: [5, 5, 5] },
  center: [0, 0, 0],
  dimensions: [10, 10, 10],
  radius: 8.660254037844386,
};

describe("mesh-render-metrics", () => {
  it("does not frame until mesh info and real viewport are available", () => {
    expect(
      meshRenderInputsReady(null, {
        width: 640,
        height: 480,
        dpr: 2,
        projectionMode: "perspective",
      }),
    ).toBe(false);
    expect(
      meshRenderInputsReady(INFO, {
        width: 0,
        height: 480,
        dpr: 2,
        projectionMode: "perspective",
      }),
    ).toBe(false);
    expect(
      meshRenderInputsReady(INFO, {
        width: 640,
        height: 0,
        dpr: 2,
        projectionMode: "perspective",
      }),
    ).toBe(false);
    expect(
      meshRenderInputsReady(INFO, {
        width: 640,
        height: 480,
        dpr: 0,
        projectionMode: "perspective",
      }),
    ).toBe(false);
    expect(
      meshRenderInputsReady(INFO, {
        width: 640,
        height: 480,
        dpr: 2,
        projectionMode: "perspective",
      }),
    ).toBe(true);
  });

  it("derives scene helper sizes from real mesh dimensions", () => {
    const metrics = meshSceneMetrics(INFO, {
      width: 640,
      height: 480,
      dpr: 2,
      projectionMode: "perspective",
    });

    expect(metrics).not.toBeNull();
    expect(metrics?.plateSize).toBeGreaterThan(200);
    expect(metrics?.gridSize).toBe(metrics?.plateSize);
    expect(metrics?.axisSize).toBeGreaterThan(0);

    const smallMetrics = meshSceneMetrics(SMALL_INFO, {
      width: 640,
      height: 480,
      dpr: 2,
      projectionMode: "perspective",
    });
    expect(smallMetrics).not.toBeNull();
    expect(metrics?.plateSize).toBeGreaterThan(smallMetrics?.plateSize ?? 0);
    expect(metrics?.gridSize).toBeGreaterThan(smallMetrics?.gridSize ?? 0);
    expect(metrics?.axisSize).toBeGreaterThan(smallMetrics?.axisSize ?? 0);
  });

  it("keeps clipping planes valid when the camera is far from the mesh", () => {
    const base = fitCameraToBounds(INFO.bounds, "iso", 1);
    const farCamera = updateCameraFromSpherical(base, { distance: 5_000 });
    const planes = clippingPlanesForBounds(farCamera, INFO.bounds);

    expect(planes.near).toBeGreaterThan(0);
    expect(planes.far).toBeGreaterThan(distanceTo(farCamera) + INFO.radius);
    expect(planes.far).toBeGreaterThan(planes.near);
  });
});

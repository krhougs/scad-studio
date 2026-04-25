import { describe, expect, it } from "vitest";
import {
  distanceTo,
  fitCameraToBounds,
  updateCameraFromSpherical,
} from "../../src/canvas/camera-controls";
import { computeMeshInfo } from "../../src/viewers/mesh-info";
import { payloadFromPreview } from "../../src/viewers/mesh-three";
import { DEFAULT_MESH_VIEWER_OPTIONS } from "../../src/viewers/viewer-options";
import {
  clippingPlanesForBounds,
  meshRenderInputsReady,
  meshSceneMetrics,
  orthographicHalfHeightForCamera,
  visibleProjectPlaneForCamera,
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

const LONG_Y_INFO: MeshInfo = {
  vertices: 8,
  indices: 36,
  bounds: { min: [-5, -200, -10], max: [5, 200, 10] },
  center: [0, 0, 0],
  dimensions: [10, 400, 20],
  radius: 200.31225624055454,
};

describe("mesh-render-metrics", () => {
  it("provides bright default preview appearance controls", () => {
    expect(DEFAULT_MESH_VIEWER_OPTIONS).toMatchObject({
      backgroundColor: "#181b20",
      gridMajorColor: "#5a6573",
      gridMinorColor: "#343b45",
      lightingIntensity: 1.25,
    });
  });

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

  it("derives projection, gizmo, fog and helper metrics from real viewport state", () => {
    const ortho = meshSceneMetrics(INFO, {
      width: 900,
      height: 600,
      dpr: 2,
      projectionMode: "orthographic",
    });
    const perspective = meshSceneMetrics(INFO, {
      width: 900,
      height: 600,
      dpr: 2,
      projectionMode: "perspective",
    });
    const highDpr = meshSceneMetrics(INFO, {
      width: 900,
      height: 600,
      dpr: 3,
      projectionMode: "orthographic",
    });
    const wide = meshSceneMetrics(INFO, {
      width: 1200,
      height: 600,
      dpr: 2,
      projectionMode: "orthographic",
    });

    expect(ortho).not.toBeNull();
    expect(perspective).not.toBeNull();
    expect(highDpr).not.toBeNull();
    expect(wide).not.toBeNull();
    expect(ortho?.orthographicHalfHeight).toBeGreaterThan(0);
    expect(perspective?.orthographicHalfHeight).toBeNull();
    expect(highDpr?.gizmoSize).toBeGreaterThan(ortho?.gizmoSize ?? 0);
    expect(wide?.orthographicHalfHeight).toBeGreaterThanOrEqual(
      INFO.dimensions[1] / 2,
    );
    expect(wide?.orthographicHalfHeight).toBeGreaterThanOrEqual(
      INFO.dimensions[2] / 2,
    );
    expect(ortho?.fogNear).toBeGreaterThan(INFO.radius);
    expect(ortho?.fogFar).toBeGreaterThan(ortho?.fogNear ?? 0);
  });

  it("keeps renderer-visible project planes tied to current camera direction", () => {
    expect(visibleProjectPlaneForCamera(fitCameraToBounds(INFO.bounds, "top", 1))).toBe(
      "xy",
    );
    expect(visibleProjectPlaneForCamera(fitCameraToBounds(INFO.bounds, "front", 1))).toBe(
      "xz",
    );
    expect(visibleProjectPlaneForCamera(fitCameraToBounds(INFO.bounds, "right", 1))).toBe(
      "yz",
    );
  });

  it("derives orthographic range from current camera screen axes", () => {
    const viewport = {
      width: 400,
      height: 800,
      dpr: 2,
      projectionMode: "orthographic" as const,
    };
    const right = orthographicHalfHeightForCamera(
      LONG_Y_INFO,
      viewport,
      fitCameraToBounds(LONG_Y_INFO.bounds, "right", 0.5),
    );
    const top = orthographicHalfHeightForCamera(
      LONG_Y_INFO,
      viewport,
      fitCameraToBounds(LONG_Y_INFO.bounds, "top", 0.5),
    );
    const front = orthographicHalfHeightForCamera(
      LONG_Y_INFO,
      viewport,
      fitCameraToBounds(LONG_Y_INFO.bounds, "front", 0.5),
    );

    expect(right).toBeGreaterThan((LONG_Y_INFO.dimensions[1] / 2 / 0.5) * 1.14);
    expect(top).toBeGreaterThan((LONG_Y_INFO.dimensions[1] / 2) * 1.14);
    expect(front).toBeLessThan(top ?? 0);
    expect(right).toBeGreaterThan(top ?? 0);
  });

  it("preserves project-coordinate mesh payload before renderer metrics consume it", () => {
    const payload = payloadFromPreview({
      artifact: {
        format: "mesh",
        payload: {
          positions: [
            [7, 0, 0],
            [0, 11, 0],
            [0, 0, 13],
          ],
          normals: [[0, 0, 1]],
          indices: [0, 1, 2],
        },
      },
    });

    expect(Array.from(payload?.positions ?? [])).toEqual([
      7, 0, 0,
      0, 11, 0,
      0, 0, 13,
    ]);
    expect(computeMeshInfo(payload?.positions ?? new Float32Array(), payload?.indices ?? null))
      .toMatchObject({
        bounds: {
          min: [0, 0, 0],
          max: [7, 11, 13],
        },
        dimensions: [7, 11, 13],
      });
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

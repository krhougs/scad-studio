import { describe, expect, it } from "vitest";
import {
  distanceTo,
  fitCameraToBounds,
  updateCameraFromSpherical,
} from "../../src/canvas/camera-controls";
import type { CameraState } from "../../src/canvas/camera-state";
import { computeMeshInfo } from "../../src/viewers/mesh-info";
import { payloadFromPreview } from "../../src/viewers/mesh-three";
import { DEFAULT_MESH_VIEWER_OPTIONS } from "../../src/viewers/viewer-options";
import {
  clippingPlanesForBounds,
  meshRenderInputsReady,
  meshSceneMetrics,
  orthographicHalfHeightForCamera,
  pointLightAutoPositionForBounds,
  visibleProjectPlaneForCamera,
} from "../../src/viewers/mesh-render-metrics";
import type { MeshBounds, MeshInfo } from "../../src/viewers/mesh-info";

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
      pointLightMode: "off",
      pointLightPosition: null,
    });
  });

  it("places automatic point light at the front view upper-right distance", () => {
    const aspectRatio = 1.5;
    const bounds: MeshBounds = { min: [10, -30, 5], max: [210, 70, 55] };
    const center = [110, 20, 30];
    const frontCamera = fitCameraToBounds(bounds, "front", aspectRatio);
    const expectedDistance = distanceTo(frontCamera);
    const position = pointLightAutoPositionForBounds(bounds, aspectRatio);
    const directionScale = expectedDistance / Math.sqrt(3);

    expect(position).toEqual([
      expect.closeTo(center[0] + directionScale, 5),
      expect.closeTo(center[1] - directionScale, 5),
      expect.closeTo(center[2] + directionScale, 5),
    ]);
    expect(
      Math.hypot(
        position[0] - center[0],
        position[1] - center[1],
        position[2] - center[2],
      ),
    ).toBeCloseTo(expectedDistance, 5);
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

  it("keeps clipping planes tight while dollying near the mesh", () => {
    const base = fitCameraToBounds(SMALL_INFO.bounds, "iso", 1);
    const nearCamera = updateCameraFromSpherical(base, {
      distance: SMALL_INFO.radius * 2.5,
    });
    const planes = clippingPlanesForBounds(nearCamera, SMALL_INFO.bounds);

    expect(planes.near).toBeGreaterThan(SMALL_INFO.radius / 20);
    expect(planes.far).toBeLessThan(120);
    expect(planes.far / planes.near).toBeLessThan(100);
    expect(planes.near).toBeLessThan(distanceTo(nearCamera) - SMALL_INFO.radius);
    expect(planes.far).toBeGreaterThan(distanceTo(nearCamera) + SMALL_INFO.radius);
  });

  it("keeps the build plate inside the far clipping plane for small meshes", () => {
    const base = fitCameraToBounds(SMALL_INFO.bounds, "iso", 1);
    const nearCamera = updateCameraFromSpherical(base, {
      distance: SMALL_INFO.radius * 2.5,
    });
    const planes = clippingPlanesForBounds(nearCamera, SMALL_INFO.bounds);
    const plateFarDepth = maxProjectedDepth(nearCamera, buildPlateCorners(SMALL_INFO));

    expect(planes.far).toBeGreaterThan(plateFarDepth);
  });

  it("does not clip a panned mesh near the camera", () => {
    const base = updateCameraFromSpherical(
      fitCameraToBounds(SMALL_INFO.bounds, "iso", 1),
      { distance: SMALL_INFO.radius * 1.8 },
    );
    const panned = panCamera(base, 40);
    const planes = clippingPlanesForBounds(panned, SMALL_INFO.bounds);
    const meshNearDepth = minProjectedDepth(panned, boundsCorners(SMALL_INFO.bounds));

    expect(planes.near).toBeLessThan(meshNearDepth);
  });
});

function buildPlateCorners(info: MeshInfo): Array<[number, number, number]> {
  const plateSize = Math.max(80, Math.max(...info.dimensions, info.radius * 2, 1) * 1.8);
  const half = plateSize / 2;
  const bottom = info.bounds.min[2] - Math.max(info.radius * 0.015, 0.02) - 0.01;
  return [
    [info.center[0] - half, info.center[1] - half, bottom],
    [info.center[0] - half, info.center[1] + half, bottom],
    [info.center[0] + half, info.center[1] - half, bottom],
    [info.center[0] + half, info.center[1] + half, bottom],
  ];
}

function boundsCorners(bounds: MeshBounds): Array<[number, number, number]> {
  return [
    [bounds.min[0], bounds.min[1], bounds.min[2]],
    [bounds.min[0], bounds.min[1], bounds.max[2]],
    [bounds.min[0], bounds.max[1], bounds.min[2]],
    [bounds.min[0], bounds.max[1], bounds.max[2]],
    [bounds.max[0], bounds.min[1], bounds.min[2]],
    [bounds.max[0], bounds.min[1], bounds.max[2]],
    [bounds.max[0], bounds.max[1], bounds.min[2]],
    [bounds.max[0], bounds.max[1], bounds.max[2]],
  ];
}

function panCamera(camera: CameraState, amount: number): CameraState {
  const forward = normalize([
    camera.target[0] - camera.position[0],
    camera.target[1] - camera.position[1],
    camera.target[2] - camera.position[2],
  ]);
  const right = normalize(cross(forward, camera.up));
  const shift = right.map((item) => item * amount) as [number, number, number];
  return {
    ...camera,
    position: add(camera.position, shift),
    target: add(camera.target, shift),
  };
}

function minProjectedDepth(
  camera: CameraState,
  points: Array<[number, number, number]>,
): number {
  return Math.min(...projectedDepths(camera, points));
}

function maxProjectedDepth(
  camera: CameraState,
  points: Array<[number, number, number]>,
): number {
  return Math.max(...projectedDepths(camera, points));
}

function projectedDepths(
  camera: CameraState,
  points: Array<[number, number, number]>,
): number[] {
  const forward = normalize([
    camera.target[0] - camera.position[0],
    camera.target[1] - camera.position[1],
    camera.target[2] - camera.position[2],
  ]);
  return points.map((point) =>
    dot(
      [
        point[0] - camera.position[0],
        point[1] - camera.position[1],
        point[2] - camera.position[2],
      ],
      forward,
    ),
  );
}

function add(
  left: [number, number, number],
  right: [number, number, number],
): [number, number, number] {
  return [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
}

function normalize(value: [number, number, number]): [number, number, number] {
  const size = Math.hypot(value[0], value[1], value[2]);
  if (size < 1e-9) return [0, 0, 0];
  return [value[0] / size, value[1] / size, value[2] / size];
}

function cross(
  left: [number, number, number],
  right: [number, number, number],
): [number, number, number] {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ];
}

function dot(
  left: [number, number, number],
  right: [number, number, number],
): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

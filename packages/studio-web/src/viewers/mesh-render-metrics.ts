import type { CameraState } from "../canvas/camera-state";
import { distanceTo, fitCameraToBounds } from "../canvas/camera-controls";
import { meshBuildPlateSize, type MeshBounds, type MeshInfo } from "./mesh-info";
import type { PointLightPosition } from "./viewer-options";

export type MeshRenderViewport = {
  width: number;
  height: number;
  dpr: number;
  projectionMode: "perspective" | "orthographic";
};

export type MeshSceneMetrics = {
  plateSize: number;
  gridSize: number;
  axisSize: number;
  visiblePlane: "xy" | "xz" | "yz";
  gizmoSize: number;
  fogNear: number;
  fogFar: number;
  orthographicHalfHeight: number | null;
};

export function meshRenderInputsReady(
  info: MeshInfo | null,
  viewport: MeshRenderViewport,
): boolean {
  return (
    info !== null &&
    Number.isFinite(viewport.width) &&
    Number.isFinite(viewport.height) &&
    Number.isFinite(viewport.dpr) &&
    viewport.width > 0 &&
    viewport.height > 0 &&
    viewport.dpr > 0
  );
}

export function meshSceneMetrics(
  info: MeshInfo | null,
  viewport: MeshRenderViewport,
): MeshSceneMetrics | null {
  if (!meshRenderInputsReady(info, viewport) || !info) return null;
  const plateSize = meshBuildPlateSize(info);
  const minViewport = Math.min(viewport.width, viewport.height);
  const aspect = Math.max(viewport.width / viewport.height, 0.1);
  const orthographicHalfHeight =
    viewport.projectionMode === "orthographic"
      ? Math.max(info.radius, info.radius / aspect, 1) * 1.15
      : null;
  return {
    plateSize,
    gridSize: plateSize,
    axisSize: Math.max(20, plateSize * 0.4),
    visiblePlane: "xy",
    gizmoSize: Math.max(24, Math.min(90, minViewport * 0.045 * viewport.dpr)),
    fogNear: Math.max(info.radius * 3, 60),
    fogFar: Math.max(info.radius * 9, 220),
    orthographicHalfHeight,
  };
}

export function orthographicHalfHeightForCamera(
  info: MeshInfo | null,
  viewport: MeshRenderViewport,
  camera: CameraState,
): number | null {
  if (!meshRenderInputsReady(info, viewport) || !info) return null;
  if (viewport.projectionMode !== "orthographic") return null;
  const aspect = Math.max(viewport.width / viewport.height, 0.1);
  const forward = normalize([
    camera.target[0] - camera.position[0],
    camera.target[1] - camera.position[1],
    camera.target[2] - camera.position[2],
  ]);
  const right = normalize(cross(forward, camera.up));
  const up = normalize(cross(right, forward));
  if (length(right) === 0 || length(up) === 0) return null;
  const center = info.center;
  let halfWidth = 0;
  let halfHeight = 0;
  for (const corner of boundsCorners(info.bounds)) {
    const relative: [number, number, number] = [
      corner[0] - center[0],
      corner[1] - center[1],
      corner[2] - center[2],
    ];
    halfWidth = Math.max(halfWidth, Math.abs(dot(relative, right)));
    halfHeight = Math.max(halfHeight, Math.abs(dot(relative, up)));
  }
  return Math.max(halfHeight, halfWidth / aspect, 1) * 1.15;
}

export function clippingPlanesForBounds(
  camera: CameraState,
  bounds: MeshBounds,
): { near: number; far: number } {
  const center: [number, number, number] = [
    (bounds.min[0] + bounds.max[0]) / 2,
    (bounds.min[1] + bounds.max[1]) / 2,
    (bounds.min[2] + bounds.max[2]) / 2,
  ];
  const dimensions: [number, number, number] = [
    Math.max(0, bounds.max[0] - bounds.min[0]),
    Math.max(0, bounds.max[1] - bounds.min[1]),
    Math.max(0, bounds.max[2] - bounds.min[2]),
  ];
  const radius = Math.max(
    Math.hypot(dimensions[0], dimensions[1], dimensions[2]) / 2,
    0.25,
  );
  const forward = normalize([
    camera.target[0] - camera.position[0],
    camera.target[1] - camera.position[1],
    camera.target[2] - camera.position[2],
  ]);
  const meshDepths = depthRangeForPoints(
    boundsCorners(bounds),
    camera.position,
    forward,
  );
  const helperDepths = depthRangeForPoints(
    buildPlateCorners(bounds, center, dimensions, radius),
    camera.position,
    forward,
  );
  const sceneNear = Math.max(0.01, radius / 20);
  const near = nearClipForDepth(meshDepths.min, sceneNear);
  const maxDepth = Math.max(meshDepths.max, helperDepths.max);
  const far = Math.max(near + 1, maxDepth + Math.max(radius * 0.5, 1));
  return { near, far };
}

export function pointLightAutoPositionForBounds(
  bounds: MeshBounds,
  aspectRatio: number,
): PointLightPosition {
  const camera = fitCameraToBounds(bounds, "front", aspectRatio);
  const distance = distanceTo(camera);
  const scale = distance / Math.sqrt(3);
  return [
    camera.target[0] + scale,
    camera.target[1] - scale,
    camera.target[2] + scale,
  ];
}

export function visibleProjectPlaneForCamera(
  camera: CameraState,
): MeshSceneMetrics["visiblePlane"] {
  const direction = [
    camera.position[0] - camera.target[0],
    camera.position[1] - camera.target[1],
    camera.position[2] - camera.target[2],
  ];
  const abs = direction.map((value) => Math.abs(value));
  if (abs[2] >= abs[0] && abs[2] >= abs[1]) return "xy";
  if (abs[0] >= abs[1]) return "yz";
  return "xz";
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

function buildPlateCorners(
  bounds: MeshBounds,
  center: [number, number, number],
  dimensions: [number, number, number],
  radius: number,
): Array<[number, number, number]> {
  const plateSize = meshBuildPlateSize({
    vertices: 0,
    indices: 0,
    bounds,
    center,
    dimensions,
    radius,
  });
  const half = plateSize / 2;
  const bottom = bounds.min[2] - Math.max(radius * 0.015, 0.02) - 0.01;
  return [
    [center[0] - half, center[1] - half, bottom],
    [center[0] - half, center[1] + half, bottom],
    [center[0] + half, center[1] - half, bottom],
    [center[0] + half, center[1] + half, bottom],
  ];
}

function depthRangeForPoints(
  points: Array<[number, number, number]>,
  cameraPosition: [number, number, number],
  forward: [number, number, number],
): { min: number; max: number } {
  let min = Infinity;
  let max = -Infinity;
  for (const point of points) {
    const depth = dot(
      [
        point[0] - cameraPosition[0],
        point[1] - cameraPosition[1],
        point[2] - cameraPosition[2],
      ],
      forward,
    );
    min = Math.min(min, depth);
    max = Math.max(max, depth);
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) return { min: 0.01, max: 1 };
  return { min, max };
}

function nearClipForDepth(meshNearDepth: number, sceneNear: number): number {
  if (meshNearDepth <= 0.02) return 0.01;
  if (meshNearDepth <= sceneNear * 2) {
    return Math.max(0.01, meshNearDepth * 0.5);
  }
  return Math.min(10, Math.max(sceneNear, meshNearDepth * 0.8));
}

function length(value: [number, number, number]): number {
  return Math.hypot(value[0], value[1], value[2]);
}

function normalize(value: [number, number, number]): [number, number, number] {
  const size = length(value);
  if (size < 1e-9) return [0, 0, 0];
  return [value[0] / size, value[1] / size, value[2] / size];
}

function cross(
  a: [number, number, number],
  b: [number, number, number],
): [number, number, number] {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

function dot(
  a: [number, number, number],
  b: [number, number, number],
): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

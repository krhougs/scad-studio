import type { CameraState } from "../canvas/camera-state";
import type { MeshBounds, MeshInfo } from "./mesh-info";

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
  _viewport: MeshRenderViewport,
): boolean {
  return info !== null;
}

export function meshSceneMetrics(
  _info: MeshInfo | null,
  _viewport: MeshRenderViewport,
): MeshSceneMetrics | null {
  return null;
}

export function clippingPlanesForBounds(
  _camera: CameraState,
  _bounds: MeshBounds,
): { near: number; far: number } {
  return { near: 0.1, far: 1000 };
}

export function visibleProjectPlaneForCamera(
  _camera: CameraState,
): MeshSceneMetrics["visiblePlane"] {
  return "xz";
}

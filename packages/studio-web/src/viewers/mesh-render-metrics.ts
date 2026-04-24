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

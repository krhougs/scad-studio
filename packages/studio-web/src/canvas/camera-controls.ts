// Pure camera transform helpers. Phase 7 intentionally keeps these as stateless
// functions so they can be exhaustively unit tested and reused across pointer
// input, toolbar buttons, and renderer driver without pulling in react.
//
// The math models an orbit camera around the target. `orbitBy` rotates around
// the target on the world Z axis (yaw) and the current right axis (pitch).
// Pitch wraps like the desktop viewer, allowing the view to cross over the
// top and bottom of the model. `panBy` moves both the
// target and the position along the camera's local right/up axes, preserving
// the orbit distance. `zoomBy` scales the position-to-target distance.

import {
  CameraPreset,
  CameraState,
  DEFAULT_FAR,
  DEFAULT_FOV_Y_DEG,
  DEFAULT_NEAR,
  PRESET_STATES,
  defaultCameraState,
  type Vec3,
} from "./camera-state";
import type { MeshBounds } from "../viewers/mesh-info";

const MIN_DIST = 1;
const MAX_DIST = 5_000;
const DEG = 180 / Math.PI;
const RAD = Math.PI / 180;
const TAU = Math.PI * 2;

function sub(a: Vec3, b: Vec3): Vec3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}
function add(a: Vec3, b: Vec3): Vec3 {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}
function scale(a: Vec3, s: number): Vec3 {
  return [a[0] * s, a[1] * s, a[2] * s];
}
function length(a: Vec3): number {
  return Math.hypot(a[0], a[1], a[2]);
}
function finiteNumber(value: number | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}
function finiteVec(value: Vec3 | undefined, fallback: Vec3): Vec3 {
  if (value && value.every((item) => Number.isFinite(item))) return value;
  return fallback;
}
function normalize(a: Vec3): Vec3 {
  const len = length(a);
  if (len < 1e-9) return [0, 0, 0];
  return [a[0] / len, a[1] / len, a[2] / len];
}
function cross(a: Vec3, b: Vec3): Vec3 {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}
function dot(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export function distanceTo(state: CameraState): number {
  return length(sub(state.position, state.target));
}

export function applyPreset(preset: CameraPreset): CameraState {
  return { ...PRESET_STATES[preset] };
}

export function resetCamera(): CameraState {
  return defaultCameraState();
}

export function zoomBy(state: CameraState, delta: number): CameraState {
  const offset = sub(state.position, state.target);
  const dist = length(offset);
  const factor = Math.min(5, Math.max(0.2, 1 - delta * 0.12));
  const next = Math.min(MAX_DIST, Math.max(MIN_DIST, dist * factor));
  const dir = dist < 1e-9 ? ([0, -1, 0] as Vec3) : normalize(offset);
  return { ...state, position: add(state.target, scale(dir, next)) };
}

export function wheelDeltaToZoomAmount(
  deltaY: number,
  deltaMode: number,
): number {
  const amount = deltaMode === 1 ? deltaY : deltaY / 120;
  return -amount;
}

export function orbitBy(
  state: CameraState,
  yawRad: number,
  pitchRad: number,
): CameraState {
  const offset = sub(state.position, state.target);
  const dist = length(offset);
  if (dist < 1e-9) return state;
  const spherical = orbitAngles(offset, state.up);
  let yaw = spherical.yaw;
  let pitch = spherical.pitch;
  yaw = wrapAngle(yaw + yawRad);
  pitch = wrapAngle(pitch + pitchRad);
  const cp = Math.cos(pitch);
  const next: Vec3 = [
    dist * cp * Math.cos(yaw),
    dist * cp * Math.sin(yaw),
    dist * Math.sin(pitch),
  ];
  return {
    ...state,
    position: add(state.target, next),
    up: orbitUp(yaw, pitch),
  };
}

export function panBy(
  state: CameraState,
  dxScreen: number,
  dyScreen: number,
): CameraState {
  const forward = normalize(sub(state.target, state.position));
  const right = normalize(cross(forward, state.up));
  const up = normalize(cross(right, forward));
  const offset = add(scale(right, dxScreen), scale(up, dyScreen));
  return {
    ...state,
    position: add(state.position, offset),
    target: add(state.target, offset),
  };
}

export type PointerMode = "orbit" | "pan" | "none";

export function classifyPointerMode(opts: {
  button: number;
  altKey: boolean;
}): PointerMode {
  if (opts.button === 1) return "pan";
  if (opts.button === 2) return "pan";
  if (opts.button === 0 && opts.altKey) return "pan";
  if (opts.button === 0) return "orbit";
  return "none";
}

export function fitCameraToBounds(
  bounds: MeshBounds | null,
  preset: CameraPreset,
  aspectRatio: number,
): CameraState {
  if (!bounds) return applyPreset(preset);
  const center: Vec3 = [
    (bounds.min[0] + bounds.max[0]) / 2,
    (bounds.min[1] + bounds.max[1]) / 2,
    (bounds.min[2] + bounds.max[2]) / 2,
  ];
  const size: Vec3 = [
    Math.max(0, bounds.max[0] - bounds.min[0]),
    Math.max(0, bounds.max[1] - bounds.min[1]),
    Math.max(0, bounds.max[2] - bounds.min[2]),
  ];
  const radius = Math.max(Math.hypot(size[0], size[1], size[2]) / 2, 0.25);
  const halfFov = (DEFAULT_FOV_Y_DEG * RAD) / 2;
  const horizontalHalfFov = Math.atan(Math.tan(halfFov) * Math.max(aspectRatio, 0.1));
  const limitingHalfFov = Math.min(halfFov, horizontalHalfFov);
  const distance = Math.min(
    MAX_DIST,
    Math.max(MIN_DIST, (radius / Math.tan(limitingHalfFov)) * 1.35),
  );
  const direction = presetDirection(preset);
  return {
    position: add(center, scale(direction, distance)),
    target: center,
    up: presetUp(preset),
    fovYDeg: DEFAULT_FOV_Y_DEG,
    near: Math.max(radius / 1000, DEFAULT_NEAR),
    far: Math.max(radius * 20, DEFAULT_FAR),
  };
}

export function sphericalFromCamera(state: CameraState): {
  target: Vec3;
  distance: number;
  azimuthDeg: number;
  elevationDeg: number;
} {
  const target = finiteVec(state.target, [0, 0, 0]);
  const position = finiteVec(state.position, [0, -MIN_DIST, 0]);
  const offset = sub(position, target);
  const distance = Math.max(length(offset), MIN_DIST);
  const spherical = orbitAngles(offset, state.up);
  const azimuthDeg = spherical.yaw * DEG;
  const elevationDeg = spherical.pitch * DEG;
  return {
    target,
    distance,
    azimuthDeg,
    elevationDeg,
  };
}

export function updateCameraFromSpherical(
  state: CameraState,
  patch: Partial<{
    target: Vec3;
    distance: number;
    azimuthDeg: number;
    elevationDeg: number;
  }>,
): CameraState {
  const current = sphericalFromCamera(state);
  const target = finiteVec(patch.target, current.target);
  const distance = Math.min(
    MAX_DIST,
    Math.max(MIN_DIST, finiteNumber(patch.distance, current.distance)),
  );
  const azimuth = finiteNumber(patch.azimuthDeg, current.azimuthDeg) * RAD;
  const elevation = wrapAngle(
    finiteNumber(patch.elevationDeg, current.elevationDeg) * RAD,
  );
  const cp = Math.cos(elevation);
  const offset: Vec3 = [
    distance * cp * Math.cos(azimuth),
    distance * cp * Math.sin(azimuth),
    distance * Math.sin(elevation),
  ];
  return {
    ...state,
    target,
    position: add(target, offset),
    up: orbitUp(azimuth, elevation),
  };
}

function orbitAngles(offset: Vec3, up: Vec3): { yaw: number; pitch: number } {
  const horizontal = Math.hypot(offset[0], offset[1]);
  if (horizontal > 1e-9) {
    const yaw = Math.atan2(offset[1], offset[0]);
    const basePitch = Math.atan2(offset[2], horizontal);
    const expectedUp = orbitUp(yaw, basePitch);
    if (dot(expectedUp, up) >= 0) return { yaw, pitch: basePitch };
    return {
      yaw: wrapAngle(yaw + Math.PI),
      pitch: wrapAngle(Math.PI - basePitch),
    };
  }
  const horizontalUp = Math.hypot(up[0], up[1]);
  if (horizontalUp > 1e-9) {
    const yaw =
      offset[2] >= 0
        ? Math.atan2(-up[1], -up[0])
        : Math.atan2(up[1], up[0]);
    return { yaw, pitch: offset[2] >= 0 ? Math.PI / 2 : -Math.PI / 2 };
  }
  return { yaw: 0, pitch: offset[2] >= 0 ? Math.PI / 2 : -Math.PI / 2 };
}

function wrapAngle(angle: number): number {
  let wrapped = angle % TAU;
  if (wrapped <= -Math.PI) wrapped += TAU;
  else if (wrapped > Math.PI) wrapped -= TAU;
  return wrapped;
}

function orbitUp(yaw: number, pitch: number): Vec3 {
  return normalize([
    -Math.sin(pitch) * Math.cos(yaw),
    -Math.sin(pitch) * Math.sin(yaw),
    Math.cos(pitch),
  ]);
}

function presetDirection(preset: CameraPreset): Vec3 {
  switch (preset) {
    case "front":
      return [0, -1, 0];
    case "back":
      return [0, 1, 0];
    case "left":
      return [-1, 0, 0];
    case "right":
      return [1, 0, 0];
    case "top":
      return [0, 0, 1];
    case "bottom":
      return [0, 0, -1];
    case "iso":
      return normalize([1, -1, 1]);
  }
}

function presetUp(preset: CameraPreset): Vec3 {
  if (preset === "top") return [0, 1, 0];
  if (preset === "bottom") return [0, -1, 0];
  return [0, 0, 1];
}

// Pure camera transform helpers. Phase 7 intentionally keeps these as stateless
// functions so they can be exhaustively unit tested and reused across pointer
// input, toolbar buttons, and renderer driver without pulling in react.
//
// The math models an orbit camera around the target. `orbitBy` rotates around
// the target on the world Z axis (yaw) and the current right axis (pitch).
// Pitch is clamped to keep the view from flipping. `panBy` moves both the
// target and the position along the camera's local right/up axes, preserving
// the orbit distance. `zoomBy` scales the position-to-target distance.

import {
  CameraPreset,
  CameraState,
  PRESET_STATES,
  defaultCameraState,
  type Vec3,
} from "./camera-state";

const MIN_DIST = 1;
const MAX_DIST = 5_000;
const MAX_PITCH = Math.PI / 2 - 0.05;

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
  const next = Math.min(MAX_DIST, Math.max(MIN_DIST, dist * Math.exp(delta)));
  const dir = dist < 1e-9 ? ([0, -1, 0] as Vec3) : normalize(offset);
  return { ...state, position: add(state.target, scale(dir, next)) };
}

export function orbitBy(
  state: CameraState,
  yawRad: number,
  pitchRad: number,
): CameraState {
  const offset = sub(state.position, state.target);
  const dist = length(offset);
  if (dist < 1e-9) return state;
  let yaw = Math.atan2(offset[1], offset[0]);
  let pitch = Math.asin(Math.max(-1, Math.min(1, offset[2] / dist)));
  yaw += yawRad;
  pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, pitch + pitchRad));
  const cp = Math.cos(pitch);
  const next: Vec3 = [
    dist * cp * Math.cos(yaw),
    dist * cp * Math.sin(yaw),
    dist * Math.sin(pitch),
  ];
  return { ...state, position: add(state.target, next) };
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
  if (opts.button === 2) return "pan";
  if (opts.button === 0 && opts.altKey) return "pan";
  if (opts.button === 0) return "orbit";
  return "none";
}

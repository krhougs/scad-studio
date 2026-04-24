// Camera state + preset constants. Kept pure so unit tests can pin the
// preset transforms without a GPU or DOM. The Three.js mesh viewer
// (viewers/mesh-three.ts) consumes these via setCamera(preset).

export type Vec3 = [number, number, number];

export type CameraState = {
  position: Vec3;
  target: Vec3;
  up: Vec3;
  fovYDeg: number;
  near: number;
  far: number;
};

export type CameraPreset =
  | "front"
  | "back"
  | "left"
  | "right"
  | "top"
  | "bottom"
  | "iso";

export const DEFAULT_FOV_Y_DEG = 35;
export const DEFAULT_NEAR = 0.1;
export const DEFAULT_FAR = 1000;
const DIST = 50;

function base(position: Vec3, up: Vec3 = [0, 0, 1]): CameraState {
  return {
    position,
    target: [0, 0, 0],
    up,
    fovYDeg: DEFAULT_FOV_Y_DEG,
    near: DEFAULT_NEAR,
    far: DEFAULT_FAR,
  };
}

export const PRESET_STATES: Record<CameraPreset, CameraState> = {
  front: base([0, -DIST, 0]),
  back: base([0, DIST, 0]),
  left: base([-DIST, 0, 0]),
  right: base([DIST, 0, 0]),
  top: base([0, 0, DIST], [0, 1, 0]),
  bottom: base([0, 0, -DIST], [0, -1, 0]),
  iso: base([DIST * 0.7, -DIST * 0.7, DIST * 0.7]),
};

export function defaultCameraState(): CameraState {
  return { ...PRESET_STATES.iso };
}

export const CAMERA_PRESETS: readonly CameraPreset[] = [
  "front",
  "back",
  "left",
  "right",
  "top",
  "bottom",
  "iso",
];

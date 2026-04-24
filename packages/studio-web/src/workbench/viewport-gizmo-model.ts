import type { CameraState, Vec3 } from "../canvas/camera-state";

export type ViewportGizmoAxis = {
  id: "x" | "y" | "z";
  start: [number, number];
  end: [number, number];
  color: string;
};

export function projectViewportGizmoAxes(
  camera: CameraState,
  size: number,
): ViewportGizmoAxis[] {
  const center = size / 2;
  const axisLength = size * 0.34;
  const forward = normalize(cameraDirection(camera));
  const up = normalize(camera.up);
  const right = normalize(cross(forward, up));
  const screenUp = normalize(cross(right, forward));
  return [
    axis("x", center, axisLength, [1, 0, 0], right, screenUp, "#ff6060"),
    axis("y", center, axisLength, [0, 1, 0], right, screenUp, "#60dc80"),
    axis("z", center, axisLength, [0, 0, 1], right, screenUp, "#6098ff"),
  ];
}

function axis(
  id: ViewportGizmoAxis["id"],
  center: number,
  axisLength: number,
  direction: Vec3,
  right: Vec3,
  screenUp: Vec3,
  color: string,
): ViewportGizmoAxis {
  const x = dot(direction, right) * axisLength;
  const y = -dot(direction, screenUp) * axisLength;
  return {
    id,
    start: [center, center],
    end: [center + x, center + y],
    color,
  };
}

export function cameraDirection(camera: CameraState): Vec3 {
  return [
    camera.target[0] - camera.position[0],
    camera.target[1] - camera.position[1],
    camera.target[2] - camera.position[2],
  ];
}

function normalize(vec: Vec3): Vec3 {
  const length = Math.hypot(vec[0], vec[1], vec[2]);
  if (length < 1e-9) return [0, 0, 0];
  return [vec[0] / length, vec[1] / length, vec[2] / length];
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

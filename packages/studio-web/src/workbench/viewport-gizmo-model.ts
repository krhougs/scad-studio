import type { CameraState, Vec3 } from "../canvas/camera-state";

export type ViewportGizmoAxis = {
  id: "x" | "y" | "z";
  start: [number, number];
  end: [number, number];
  color: string;
};

export function projectViewportGizmoAxes(
  _camera: CameraState,
  size: number,
): ViewportGizmoAxis[] {
  const center = size / 2;
  return [
    axis("x", center, "#ff6060"),
    axis("y", center, "#60dc80"),
    axis("z", center, "#6098ff"),
  ];
}

function axis(
  id: ViewportGizmoAxis["id"],
  center: number,
  color: string,
): ViewportGizmoAxis {
  return {
    id,
    start: [center, center],
    end: [center, center],
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

// Canvas status bar: shows camera position / fov / active preset. Kept
// intentionally dumb; the parent CanvasZone hands us the camera state and we
// format it for display.

import type { CameraPreset, CameraState } from "../canvas/camera-state";

type CanvasStatusbarProps = {
  camera: CameraState;
  activePreset: CameraPreset | null;
};

function fmt(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return value.toFixed(1);
}

export function CanvasStatusbar({ camera, activePreset }: CanvasStatusbarProps) {
  const [px, py, pz] = camera.position;
  return (
    <div
      className="canvas-statusbar"
      data-testid="canvas-statusbar"
      aria-live="polite"
    >
      <span className="canvas-statusbar__cell">
        position
        <span data-testid="canvas-status-position">
          {`(${fmt(px)}, ${fmt(py)}, ${fmt(pz)})`}
        </span>
      </span>
      <span className="canvas-statusbar__cell">
        fov
        <span data-testid="canvas-status-fov">{fmt(camera.fovYDeg)}</span>
      </span>
      <span className="canvas-statusbar__cell">
        preset
        <span data-testid="canvas-status-preset">
          {activePreset ?? "custom"}
        </span>
      </span>
    </div>
  );
}

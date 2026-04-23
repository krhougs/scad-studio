// Canvas toolbar: camera preset buttons + zoom/reset controls. The toolbar is
// presentational only; camera state lives in the CanvasZone and is advanced
// through the pure helpers in canvas/camera-controls.ts. Sentence-case labels
// and hairline borders follow docs/design-system/studio-datasheet-workbench.md.

import { CAMERA_PRESETS, type CameraPreset } from "../canvas/camera-state";

type CanvasToolbarProps = {
  activePreset: CameraPreset | null;
  onApplyPreset: (preset: CameraPreset) => void;
  onReset: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
};

const PRESET_LABEL: Record<CameraPreset, string> = {
  front: "front",
  back: "back",
  left: "left",
  right: "right",
  top: "top",
  bottom: "bottom",
  iso: "iso",
};

export function CanvasToolbar({
  activePreset,
  onApplyPreset,
  onReset,
  onZoomIn,
  onZoomOut,
}: CanvasToolbarProps) {
  return (
    <div
      className="canvas-toolbar"
      aria-label="camera controls"
      data-testid="canvas-toolbar"
    >
      <div className="canvas-toolbar__group" role="group" aria-label="presets">
        {CAMERA_PRESETS.map((preset) => {
          const isActive = preset === activePreset;
          return (
            <button
              key={preset}
              type="button"
              className={`btn btn--sm${isActive ? " is-active" : ""}`}
              onClick={() => onApplyPreset(preset)}
              data-testid={`canvas-preset-${preset}`}
              aria-pressed={isActive}
            >
              {PRESET_LABEL[preset]}
            </button>
          );
        })}
      </div>
      <div className="canvas-toolbar__group" role="group" aria-label="zoom">
        <button
          type="button"
          className="btn btn--sm"
          onClick={onZoomOut}
          data-testid="canvas-zoom-out"
        >
          −
        </button>
        <button
          type="button"
          className="btn btn--sm"
          onClick={onZoomIn}
          data-testid="canvas-zoom-in"
        >
          +
        </button>
        <button
          type="button"
          className="btn btn--sm"
          onClick={onReset}
          data-testid="canvas-reset"
        >
          reset
        </button>
      </div>
    </div>
  );
}

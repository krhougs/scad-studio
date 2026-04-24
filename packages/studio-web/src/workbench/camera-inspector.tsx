import {
  fitCameraToBounds,
  sphericalFromCamera,
  updateCameraFromSpherical,
} from "../canvas/camera-controls";
import type { CameraPreset, CameraState } from "../canvas/camera-state";
import type { MeshInfo, Vec3 } from "../viewers/mesh-info";
import { NumericControl } from "./numeric-control";

type CameraInspectorProps = {
  camera: CameraState | null;
  meshInfo: MeshInfo | null;
  onChange: (camera: CameraState) => void;
};

const PRESETS: CameraPreset[] = [
  "front",
  "back",
  "left",
  "right",
  "top",
  "bottom",
  "iso",
];

export function CameraInspector({
  camera,
  meshInfo,
  onChange,
}: CameraInspectorProps) {
  if (!camera) {
    return <p className="panel__empty">camera pending.</p>;
  }
  const spherical = sphericalFromCamera(camera);
  const updateSpherical = (
    patch: Parameters<typeof updateCameraFromSpherical>[1],
  ) => onChange(updateCameraFromSpherical(camera, patch));
  const setTarget = (index: 0 | 1 | 2, value: number) => {
    const target: Vec3 = [...camera.target];
    target[index] = value;
    updateSpherical({ target });
  };
  const fitPreset = (preset: CameraPreset) => {
    onChange(fitCameraToBounds(meshInfo?.bounds ?? null, preset, 1));
  };

  return (
    <div className="camera-panel" data-testid="camera-panel">
      <NumberField
        label="target x"
        value={camera.target[0]}
        testId="camera-target-x"
        min={-500}
        max={500}
        step={0.1}
        onChange={(value) => setTarget(0, value)}
      />
      <NumberField
        label="target y"
        value={camera.target[1]}
        testId="camera-target-y"
        min={-500}
        max={500}
        step={0.1}
        onChange={(value) => setTarget(1, value)}
      />
      <NumberField
        label="target z"
        value={camera.target[2]}
        testId="camera-target-z"
        min={-500}
        max={500}
        step={0.1}
        onChange={(value) => setTarget(2, value)}
      />
      <NumberField
        label="distance"
        value={spherical.distance}
        testId="camera-distance"
        min={1}
        max={5000}
        step={1}
        onChange={(distance) => updateSpherical({ distance })}
      />
      <NumberField
        label="azimuth"
        value={spherical.azimuthDeg}
        testId="camera-azimuth"
        min={-180}
        max={180}
        step={1}
        onChange={(azimuthDeg) => updateSpherical({ azimuthDeg })}
      />
      <NumberField
        label="elevation"
        value={spherical.elevationDeg}
        testId="camera-elevation"
        min={-89.9}
        max={89.9}
        step={1}
        onChange={(elevationDeg) => updateSpherical({ elevationDeg })}
      />
      <div className="camera-panel__presets">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={() => fitPreset("iso")}
          data-testid="camera-reset"
        >
          reset
        </button>
        {PRESETS.map((preset) => (
          <button
            key={preset}
            type="button"
            className="btn btn--line btn--sm"
            onClick={() => fitPreset(preset)}
            data-testid={`camera-preset-${preset}`}
          >
            {preset}
          </button>
        ))}
      </div>
    </div>
  );
}

function NumberField({
  label,
  value,
  testId,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  testId: string;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  const id = testId.replace(/^camera-/, "");
  return (
    <div className="camera-panel__field" data-testid={`camera-control-${id}`}>
      <span className="camera-panel__field-label">{label}</span>
      <NumericControl
        label={label}
        value={value}
        min={min}
        max={max}
        step={step}
        inputTestId={testId}
        knobTestId={`camera-knob-${id}`}
        numberFieldTestId={`camera-number-field-${id}`}
        fractionDigits={3}
        onChange={onChange}
      />
    </div>
  );
}

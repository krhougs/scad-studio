import { useEffect, useState } from "react";
import {
  fitCameraToBounds,
  sphericalFromCamera,
  updateCameraFromSpherical,
} from "../canvas/camera-controls";
import type { CameraPreset, CameraState } from "../canvas/camera-state";
import type { MeshInfo, Vec3 } from "../viewers/mesh-info";

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
        onChange={(value) => setTarget(0, value)}
      />
      <NumberField
        label="target y"
        value={camera.target[1]}
        testId="camera-target-y"
        onChange={(value) => setTarget(1, value)}
      />
      <NumberField
        label="target z"
        value={camera.target[2]}
        testId="camera-target-z"
        onChange={(value) => setTarget(2, value)}
      />
      <NumberField
        label="distance"
        value={spherical.distance}
        testId="camera-distance"
        onChange={(distance) => updateSpherical({ distance })}
      />
      <NumberField
        label="azimuth"
        value={spherical.azimuthDeg}
        testId="camera-azimuth"
        onChange={(azimuthDeg) => updateSpherical({ azimuthDeg })}
      />
      <NumberField
        label="elevation"
        value={spherical.elevationDeg}
        testId="camera-elevation"
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
  onChange,
}: {
  label: string;
  value: number;
  testId: string;
  onChange: (value: number) => void;
}) {
  const formatted = Number.isFinite(value) ? value.toFixed(3) : "0";
  const [draft, setDraft] = useState(formatted);

  useEffect(() => {
    setDraft(formatted);
  }, [formatted]);

  return (
    <label className="camera-panel__field">
      <span>{label}</span>
      <input
        type="number"
        value={draft}
        step="0.01"
        onBlur={() => {
          if (draft.trim() === "" || !Number.isFinite(Number(draft))) {
            setDraft(formatted);
          }
        }}
        onChange={(event) => {
          const next = event.target.value;
          setDraft(next);
          if (next.trim() === "") return;
          const parsed = Number(next);
          if (Number.isFinite(parsed)) onChange(parsed);
        }}
        data-testid={testId}
      />
    </label>
  );
}

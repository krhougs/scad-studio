import type {
  PointLightMode,
  PointLightPosition,
  PreviewAppearance,
} from "../viewers/viewer-options";
import { NumericControl } from "./numeric-control";

type PreviewAppearancePanelProps = {
  appearance: PreviewAppearance;
  autoPointLightPosition: PointLightPosition | null;
  onChange: (patch: Partial<PreviewAppearance>) => void;
};

export function PreviewAppearancePanel({
  appearance,
  autoPointLightPosition,
  onChange,
}: PreviewAppearancePanelProps) {
  const manualPosition = pointLightPositionOrFallback(
    appearance.pointLightPosition,
    autoPointLightPosition,
  );
  const canSaveManualPosition =
    appearance.pointLightPosition !== null || autoPointLightPosition !== null;
  const setMode = (pointLightMode: PointLightMode) => {
    if (pointLightMode !== "manual") {
      onChange({ pointLightMode });
      return;
    }
    if (!canSaveManualPosition) return;
    onChange({
      pointLightMode,
      pointLightPosition: manualPosition,
    });
  };
  const setManualAxis = (axis: 0 | 1 | 2, value: number) => {
    const next: PointLightPosition = [...manualPosition];
    next[axis] = value;
    onChange({
      pointLightMode: "manual",
      pointLightPosition: next,
    });
  };
  const resetManualPosition = () => {
    if (!autoPointLightPosition) return;
    onChange({
      pointLightMode: "manual",
      pointLightPosition: autoPointLightPosition,
    });
  };

  return (
    <div className="preview-appearance-panel" data-testid="preview-appearance-panel">
      <ColorField
        label="background"
        value={appearance.backgroundColor}
        testId="preview-background-color"
        onChange={(backgroundColor) => onChange({ backgroundColor })}
      />
      <ColorField
        label="grid major"
        value={appearance.gridMajorColor}
        testId="preview-grid-major-color"
        onChange={(gridMajorColor) => onChange({ gridMajorColor })}
      />
      <ColorField
        label="grid minor"
        value={appearance.gridMinorColor}
        testId="preview-grid-minor-color"
        onChange={(gridMinorColor) => onChange({ gridMinorColor })}
      />
      <div className="preview-appearance-panel__field">
        <span className="preview-appearance-panel__field-label">lighting</span>
        <NumericControl
          label="lighting"
          value={appearance.lightingIntensity}
          min={0.25}
          max={3}
          step={0.05}
          inputTestId="preview-lighting-intensity"
          knobTestId="preview-lighting-knob"
          numberFieldTestId="preview-lighting-number-field"
          fractionDigits={2}
          onChange={(lightingIntensity) => onChange({ lightingIntensity })}
        />
      </div>
      <div className="preview-appearance-panel__field">
        <span className="preview-appearance-panel__field-label">point light</span>
        <NumericControl
          label="point light intensity"
          value={appearance.pointLightIntensity}
          min={0}
          max={5}
          step={0.05}
          inputTestId="preview-point-light-intensity"
          knobTestId="preview-point-light-intensity-knob"
          numberFieldTestId="preview-point-light-intensity-number-field"
          fractionDigits={2}
          onChange={(pointLightIntensity) => onChange({ pointLightIntensity })}
        />
      </div>
      <div className="preview-appearance-panel__field">
        <span className="preview-appearance-panel__field-label">point mode</span>
        <div className="preview-appearance-panel__segmented">
          {(["off", "auto", "manual"] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              className={
                appearance.pointLightMode === mode
                  ? "preview-appearance-panel__mode is-active"
                  : "preview-appearance-panel__mode"
              }
              data-testid={`preview-point-light-mode-${mode}`}
              aria-pressed={appearance.pointLightMode === mode}
              disabled={mode === "manual" && !canSaveManualPosition}
              onClick={() => setMode(mode)}
            >
              {mode}
            </button>
          ))}
        </div>
      </div>
      {appearance.pointLightMode === "manual" && canSaveManualPosition ? (
        <>
          <PointLightAxisField
            axis="x"
            value={manualPosition[0]}
            onChange={(value) => setManualAxis(0, value)}
          />
          <PointLightAxisField
            axis="y"
            value={manualPosition[1]}
            onChange={(value) => setManualAxis(1, value)}
          />
          <PointLightAxisField
            axis="z"
            value={manualPosition[2]}
            onChange={(value) => setManualAxis(2, value)}
          />
          <div className="preview-appearance-panel__field">
            <span className="preview-appearance-panel__field-label">position</span>
            <button
              type="button"
              className="preview-appearance-panel__reset"
              data-testid="preview-point-light-reset"
              disabled={!autoPointLightPosition}
              onClick={resetManualPosition}
            >
              reset
            </button>
          </div>
        </>
      ) : appearance.pointLightMode === "manual" ? (
        <div className="preview-appearance-panel__field">
          <span className="preview-appearance-panel__field-label">position</span>
          <button
            type="button"
            className="preview-appearance-panel__reset"
            data-testid="preview-point-light-reset"
            disabled
          >
            waiting
          </button>
        </div>
      ) : null}
    </div>
  );
}

function PointLightAxisField({
  axis,
  value,
  onChange,
}: {
  axis: "x" | "y" | "z";
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="preview-appearance-panel__field">
      <span className="preview-appearance-panel__field-label">{axis}</span>
      <NumericControl
        label={`point light ${axis}`}
        value={value}
        min={-5000}
        max={5000}
        step={0.1}
        inputTestId={`preview-point-light-${axis}-input`}
        knobTestId={`preview-point-light-${axis}-knob`}
        numberFieldTestId={`preview-point-light-${axis}-number-field`}
        fractionDigits={3}
        onChange={onChange}
      />
    </div>
  );
}

function pointLightPositionOrFallback(
  primary: PointLightPosition | null,
  fallback: PointLightPosition | null,
): PointLightPosition {
  return primary ?? fallback ?? [0, -160, 160];
}

function ColorField({
  label,
  value,
  testId,
  onChange,
}: {
  label: string;
  value: string;
  testId: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="preview-appearance-panel__field">
      <span className="preview-appearance-panel__field-label">{label}</span>
      <input
        className="preview-appearance-panel__color"
        type="color"
        value={value}
        data-testid={testId}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

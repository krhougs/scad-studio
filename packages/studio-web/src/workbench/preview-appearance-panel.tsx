import type { PreviewAppearance } from "../viewers/viewer-options";
import { NumericControl } from "./numeric-control";

type PreviewAppearancePanelProps = {
  appearance: PreviewAppearance;
  onChange: (patch: Partial<PreviewAppearance>) => void;
};

export function PreviewAppearancePanel({
  appearance,
  onChange,
}: PreviewAppearancePanelProps) {
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
    </div>
  );
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

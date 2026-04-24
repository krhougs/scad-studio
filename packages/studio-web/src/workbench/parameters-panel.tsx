import {
  choiceOptions,
  parameterKind,
  sliderBounds,
  type ParameterEntry,
  type ParameterValue,
} from "./parameter-model";
import { useState } from "react";
import { NumericControl } from "./numeric-control";

type ParametersPanelProps = {
  entries: ParameterEntry[];
  warnings: string[];
  onUpdateValue: (name: string, value: ParameterValue) => void;
  onRestoreValue: (name: string) => void;
  onRestoreDefaults: () => void;
  onSavePreset: (name: string) => void;
  previewStatus: string;
};

export function ParametersPanel({
  entries,
  warnings,
  onUpdateValue,
  onRestoreValue,
  onRestoreDefaults,
  onSavePreset,
  previewStatus,
}: ParametersPanelProps) {
  const visibleEntries = entries.filter((entry) => !entry.definition.hidden);
  const [draftName, setDraftName] = useState("");

  return (
    <section
      className="panel panel--parameters"
      aria-label="parameters"
      data-testid="parameters-panel"
    >
      <header className="panel__head">
        <h5 className="panel__title">parameters</h5>
        <span className="panel__sub" data-testid="parameters-status">
          {previewStatus}
        </span>
      </header>
      <ul className="panel__list">
        {visibleEntries.map((entry) => (
          <li
            key={entry.definition.name}
            className="panel__row parameter-row"
            data-testid={`parameter-row-${entry.definition.name}`}
          >
            <span className="panel__label">{entry.definition.name}</span>
            <ParameterControl entry={entry} onUpdateValue={onUpdateValue} />
            <button
              type="button"
              className="btn btn--ghost btn--sm parameter-restore"
              onClick={() => onRestoreValue(entry.definition.name)}
              data-testid={`parameter-restore-${entry.definition.name}`}
              disabled={entry.value === entry.definition.default_value}
            >
              restore
            </button>
          </li>
        ))}
        {visibleEntries.length === 0 ? (
          <li className="panel__empty" data-testid="parameters-empty">
            no Customizer parameters detected.
          </li>
        ) : null}
      </ul>
      {warnings.length > 0 ? (
        <ul className="panel__list" data-testid="parameters-warnings">
          {warnings.map((warning) => (
            <li key={warning} className="panel__empty">
              {warning}
            </li>
          ))}
        </ul>
      ) : null}
      <div className="panel__actions">
        <input
          type="text"
          className="panel__input"
          placeholder="preset name"
          value={draftName}
          onChange={(ev) => setDraftName(ev.target.value)}
          data-testid="preset-save-name"
        />
        <button
          type="button"
          className="btn btn--solid btn--sm"
          onClick={() => {
            const name = draftName.trim();
            if (name.length === 0) return;
            onSavePreset(name);
            setDraftName("");
          }}
          data-testid="preset-save"
        >
          save preset
        </button>
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={onRestoreDefaults}
          data-testid="parameters-restore"
        >
          restore defaults
        </button>
      </div>
    </section>
  );
}

function ParameterControl({
  entry,
  onUpdateValue,
}: {
  entry: ParameterEntry;
  onUpdateValue: (name: string, value: ParameterValue) => void;
}) {
  const name = entry.definition.name;
  const testId = `parameter-control-${name}`;
  const kind = parameterKind(entry);
  if (kind === "number") {
    const range = sliderBounds(entry);
    const value = typeof entry.value === "number" ? entry.value : 0;
    return (
      <NumericControl
        label={name}
        value={value}
        min={range.min}
        max={range.max}
        step={range.step}
        inputTestId={testId}
        knobTestId={`parameter-knob-${name}`}
        numberFieldTestId={`parameter-number-field-${name}`}
        onChange={(next) => onUpdateValue(name, next)}
      />
    );
  }
  if (kind === "bool") {
    return (
      <input
        type="checkbox"
        checked={entry.value === true}
        onChange={(ev) => onUpdateValue(name, ev.target.checked)}
        data-testid={testId}
      />
    );
  }
  if (kind === "choice") {
    return (
      <select
        className="panel__input"
        value={String(entry.value)}
        onChange={(ev) => onUpdateValue(name, ev.target.value)}
        data-testid={testId}
      >
        {choiceOptions(entry).map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      type="text"
      className="panel__input"
      value={String(entry.value)}
      onChange={(ev) => onUpdateValue(name, ev.target.value)}
      data-testid={testId}
    />
  );
}

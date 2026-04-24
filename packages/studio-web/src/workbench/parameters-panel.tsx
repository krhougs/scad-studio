import {
  choiceOptions,
  numberBounds,
  parameterKind,
  type ParameterEntry,
  type ParameterValue,
} from "./parameter-model";

type ParametersPanelProps = {
  entries: ParameterEntry[];
  warnings: string[];
  onUpdateValue: (name: string, value: ParameterValue) => void;
  onRestoreValue: (name: string) => void;
  onApply: () => void;
  onRestoreDefaults: () => void;
  previewStatus: string;
};

export function ParametersPanel({
  entries,
  warnings,
  onUpdateValue,
  onRestoreValue,
  onApply,
  onRestoreDefaults,
  previewStatus,
}: ParametersPanelProps) {
  const visibleEntries = entries.filter((entry) => !entry.definition.hidden);

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
            className="panel__row"
            data-testid={`parameter-row-${entry.definition.name}`}
          >
            <span className="panel__label">{entry.definition.name}</span>
            <ParameterControl entry={entry} onUpdateValue={onUpdateValue} />
            {entry.value !== entry.definition.default_value ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() => onRestoreValue(entry.definition.name)}
                data-testid={`parameter-restore-${entry.definition.name}`}
              >
                restore
              </button>
            ) : null}
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
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={onRestoreDefaults}
          data-testid="parameters-restore"
        >
          restore defaults
        </button>
        <button
          type="button"
          className="btn btn--solid btn--sm"
          onClick={onApply}
          data-testid="parameters-apply"
        >
          apply
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
    const bounds = numberBounds(entry);
    return (
      <input
        type="number"
        className="panel__input"
        value={String(entry.value)}
        min={bounds.min}
        max={bounds.max}
        step={bounds.step}
        onChange={(ev) => onUpdateValue(name, Number(ev.target.value))}
        data-testid={testId}
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

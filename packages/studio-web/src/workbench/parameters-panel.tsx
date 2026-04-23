// Parameters panel: lets the user author OpenSCAD `-D name=value` overrides
// that are sent with the next PreviewRequest. Web clients cannot read .scad
// source through FileRead (server deny list, see docs/web-platform-limits.md),
// so there is no automatic source parsing here; overrides are authored
// manually. The panel is a stateless UI layer; state lives in the parent
// (ScadSplitViewer) alongside the preview request pipeline.

import { useState } from "react";

export type ParameterOverride = {
  id: string;
  name: string;
  value: string;
};

type ParametersPanelProps = {
  overrides: ParameterOverride[];
  onAddOverride: (name: string, value: string) => void;
  onUpdateOverride: (id: string, patch: Partial<ParameterOverride>) => void;
  onRemoveOverride: (id: string) => void;
  onApply: () => void;
  onRestoreDefaults: () => void;
  previewStatus: string;
};

export function ParametersPanel({
  overrides,
  onAddOverride,
  onUpdateOverride,
  onRemoveOverride,
  onApply,
  onRestoreDefaults,
  previewStatus,
}: ParametersPanelProps) {
  const [draftName, setDraftName] = useState("");
  const [draftValue, setDraftValue] = useState("");

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
        {overrides.map((entry) => (
          <li
            key={entry.id}
            className="panel__row"
            data-testid={`parameter-row-${entry.name}`}
          >
            <input
              type="text"
              className="panel__input"
              value={entry.name}
              onChange={(ev) =>
                onUpdateOverride(entry.id, { name: ev.target.value })
              }
              aria-label="parameter name"
              data-testid={`parameter-name-${entry.id}`}
            />
            <input
              type="text"
              className="panel__input"
              value={entry.value}
              onChange={(ev) =>
                onUpdateOverride(entry.id, { value: ev.target.value })
              }
              aria-label="parameter value"
              data-testid={`parameter-value-${entry.id}`}
            />
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={() => onRemoveOverride(entry.id)}
              data-testid={`parameter-remove-${entry.id}`}
              aria-label={`remove ${entry.name}`}
            >
              remove
            </button>
          </li>
        ))}
        {overrides.length === 0 ? (
          <li className="panel__empty" data-testid="parameters-empty">
            no overrides. click "add" to set a define.
          </li>
        ) : null}
      </ul>
      <div className="panel__row panel__row--compose">
        <input
          type="text"
          className="panel__input"
          placeholder="name"
          value={draftName}
          onChange={(ev) => setDraftName(ev.target.value)}
          data-testid="parameter-draft-name"
        />
        <input
          type="text"
          className="panel__input"
          placeholder="value"
          value={draftValue}
          onChange={(ev) => setDraftValue(ev.target.value)}
          data-testid="parameter-draft-value"
        />
        <button
          type="button"
          className="btn btn--line btn--sm"
          onClick={() => {
            const name = draftName.trim();
            if (name.length === 0) return;
            onAddOverride(name, draftValue);
            setDraftName("");
            setDraftValue("");
          }}
          data-testid="parameter-add"
        >
          add
        </button>
      </div>
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

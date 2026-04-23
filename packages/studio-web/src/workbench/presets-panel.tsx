// Presets panel: lists saved parameter presets for the current .scad file and
// offers save / load / delete. Presets persist as a JSON sibling file
// (<source>.presets.json) and round-trip through FileRead / FileWriteText.
// All state is component-local; we never cache presets in the Zustand store.

import { useState } from "react";

export type PresetEntry = {
  name: string;
  defines: string[];
};

type PresetsPanelProps = {
  presetPath: string;
  presets: PresetEntry[];
  loading: boolean;
  error: string | null;
  onReload: () => void;
  onLoadPreset: (name: string) => void;
  onSavePreset: (name: string) => void;
  onDeletePreset: (name: string) => void;
};

export function PresetsPanel({
  presetPath,
  presets,
  loading,
  error,
  onReload,
  onLoadPreset,
  onSavePreset,
  onDeletePreset,
}: PresetsPanelProps) {
  const [draftName, setDraftName] = useState("");
  return (
    <section
      className="panel panel--presets"
      aria-label="presets"
      data-testid="presets-panel"
    >
      <header className="panel__head">
        <h5 className="panel__title">presets</h5>
        <span
          className="panel__sub"
          data-testid="presets-path"
          title={presetPath}
        >
          {presetPath}
        </span>
      </header>
      {loading ? (
        <p className="panel__empty" data-testid="presets-loading">
          loading…
        </p>
      ) : null}
      {error ? (
        <p className="panel__empty" data-testid="presets-error">
          {error}
        </p>
      ) : null}
      <ul className="panel__list">
        {presets.map((preset) => (
          <li
            key={preset.name}
            className="panel__row"
            data-testid={`preset-row-${preset.name}`}
          >
            <span className="panel__label" data-testid={`preset-name-${preset.name}`}>
              {preset.name}
            </span>
            <span className="panel__meta">
              {preset.defines.length} define{preset.defines.length === 1 ? "" : "s"}
            </span>
            <button
              type="button"
              className="btn btn--line btn--sm"
              onClick={() => onLoadPreset(preset.name)}
              data-testid={`preset-load-${preset.name}`}
            >
              load
            </button>
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={() => onDeletePreset(preset.name)}
              data-testid={`preset-delete-${preset.name}`}
              aria-label={`delete ${preset.name}`}
            >
              delete
            </button>
          </li>
        ))}
        {!loading && !error && presets.length === 0 ? (
          <li className="panel__empty" data-testid="presets-empty">
            no presets saved.
          </li>
        ) : null}
      </ul>
      <div className="panel__row panel__row--compose">
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
          save
        </button>
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={onReload}
          data-testid="preset-reload"
        >
          reload
        </button>
      </div>
    </section>
  );
}

import { useCallback, useEffect, useState } from "react";
import {
  type AppConfigShape,
  type AppConfigState,
  type DisplayUnit,
  normalizeAppConfig,
  type SlicerRow,
} from "../config/app-config";
import { setAppConfigReady } from "../config/app-config-store";
import type { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "../viewers/file-read-decoder";
import { SidePanelHeader } from "./side-panel-header";

type SettingsPanelProps = {
  client: WasmClient | null;
  appConfig: AppConfigState;
  wsUrl: string;
};

export function SettingsPanel({ client, appConfig, wsUrl }: SettingsPanelProps) {
  const [openscadPath, setOpenscadPath] = useState("");
  const [floatingPanelOpacity, setFloatingPanelOpacity] = useState("0.85");
  const [displayUnit, setDisplayUnit] = useState<DisplayUnit>("millimeter");
  const [slicers, setSlicers] = useState<SlicerRow[]>([]);
  const [draftSlicerName, setDraftSlicerName] = useState("");
  const [draftSlicerPath, setDraftSlicerPath] = useState("");
  const [saveStatus, setSaveStatus] = useState("idle");

  useEffect(() => {
    if (appConfig.kind !== "ready") return;
    setOpenscadPath(appConfig.config.openscad_path ?? "");
    setFloatingPanelOpacity(
      String(appConfig.config.floating_panel_opacity ?? 0.85),
    );
    setDisplayUnit(appConfig.config.display_unit ?? "millimeter");
    setSlicers(appConfig.config.slicers ?? []);
    setSaveStatus(appConfig.source === "save" ? "saved" : "idle");
  }, [appConfig]);

  const readyConfig = appConfig.kind === "ready" ? appConfig.config : null;

  const save = useCallback(async () => {
    if (!client || !readyConfig) return;
    setSaveStatus("saving");
    const next = normalizeAppConfig({
      ...readyConfig,
      openscad_path: openscadPath,
      slicers,
      floating_panel_opacity: parseOpacity(
        floatingPanelOpacity,
        readyConfig.floating_panel_opacity,
      ),
      display_unit: displayUnit,
    });
    const raw = JSON.stringify(next);
    try {
      await client.dispatchConfigSave({ json: raw });
      setSaveStatus("saved");
      setAppConfigReady(next, raw, "save");
    } catch (err) {
      setSaveStatus(`save error: ${describeFileReadError(err)}`);
    }
  }, [client, displayUnit, floatingPanelOpacity, openscadPath, readyConfig, slicers]);

  const addSlicer = useCallback(() => {
    const name = draftSlicerName.trim();
    const path = draftSlicerPath.trim();
    if (!name || !path) return;
    setSlicers((prev) => [
      ...prev.filter((item) => item.name !== name),
      { name, path },
    ]);
    setDraftSlicerName("");
    setDraftSlicerPath("");
  }, [draftSlicerName, draftSlicerPath]);

  const updateSlicer = useCallback((index: number, patch: Partial<SlicerRow>) => {
    setSlicers((prev) =>
      prev.map((entry, current) =>
        current === index ? { ...entry, ...patch } : entry,
      ),
    );
  }, []);

  const removeSlicer = useCallback((index: number) => {
    setSlicers((prev) => prev.filter((_, current) => current !== index));
  }, []);

  return (
    <section
      className="side-panel side-panel--settings side-panel--flush"
      data-testid="left-panel-settings"
      aria-label="settings"
    >
      <SidePanelHeader title="settings" meta={saveStatus === "saved" ? "saved" : "app server config"} />
      <div className="side-panel__body settings-panel">
        {appConfig.kind === "idle" || appConfig.kind === "loading" ? (
          <p className="side-panel__empty" data-testid="settings-loading">
            loading config…
          </p>
        ) : null}
        {appConfig.kind === "error" ? (
          <p className="side-panel__empty is-error" data-testid="settings-error">
            error: {appConfig.message}
          </p>
        ) : null}
        {readyConfig ? (
          <SettingsForm
            openscadPath={openscadPath}
            setOpenscadPath={setOpenscadPath}
            floatingPanelOpacity={floatingPanelOpacity}
            setFloatingPanelOpacity={setFloatingPanelOpacity}
            displayUnit={displayUnit}
            setDisplayUnit={setDisplayUnit}
            slicers={slicers}
            draftSlicerName={draftSlicerName}
            setDraftSlicerName={setDraftSlicerName}
            draftSlicerPath={draftSlicerPath}
            setDraftSlicerPath={setDraftSlicerPath}
            updateSlicer={updateSlicer}
            removeSlicer={removeSlicer}
            addSlicer={addSlicer}
            save={save}
            saveDisabled={!client}
            saveStatus={saveStatus}
            config={readyConfig}
          />
        ) : null}
        <p className="settings-panel__ws">
          WebSocket URL: <code>{wsUrl}</code>
        </p>
      </div>
    </section>
  );
}

type SettingsFormProps = {
  openscadPath: string;
  setOpenscadPath: (value: string) => void;
  floatingPanelOpacity: string;
  setFloatingPanelOpacity: (value: string) => void;
  displayUnit: DisplayUnit;
  setDisplayUnit: (value: DisplayUnit) => void;
  slicers: SlicerRow[];
  draftSlicerName: string;
  setDraftSlicerName: (value: string) => void;
  draftSlicerPath: string;
  setDraftSlicerPath: (value: string) => void;
  updateSlicer: (index: number, patch: Partial<SlicerRow>) => void;
  removeSlicer: (index: number) => void;
  addSlicer: () => void;
  save: () => void;
  saveDisabled: boolean;
  saveStatus: string;
  config: AppConfigShape;
};

function SettingsForm(props: SettingsFormProps) {
  const {
    openscadPath,
    setOpenscadPath,
    floatingPanelOpacity,
    setFloatingPanelOpacity,
    displayUnit,
    setDisplayUnit,
    slicers,
    draftSlicerName,
    setDraftSlicerName,
    draftSlicerPath,
    setDraftSlicerPath,
    updateSlicer,
    removeSlicer,
    addSlicer,
    save,
    saveDisabled,
    saveStatus,
    config,
  } = props;
  return (
    <div className="settings-panel__form">
      <label className="settings-panel__field">
        <span>openscad path</span>
        <input
          type="text"
          value={openscadPath}
          onChange={(ev) => setOpenscadPath(ev.target.value)}
          data-testid="settings-openscad-path"
        />
      </label>
      <label className="settings-panel__field">
        <span>floating panel opacity</span>
        <input
          type="number"
          min="0"
          max="1"
          step="0.01"
          value={floatingPanelOpacity}
          onChange={(ev) => setFloatingPanelOpacity(ev.target.value)}
          data-testid="settings-floating-panel-opacity"
        />
      </label>
      <label className="settings-panel__field">
        <span>display unit</span>
        <select
          value={displayUnit}
          onChange={(ev) => setDisplayUnit(ev.target.value as DisplayUnit)}
          data-testid="settings-display-unit"
        >
          <option value="millimeter">mm</option>
          <option value="centimeter">cm</option>
          <option value="inch">in</option>
        </select>
      </label>
      <div className="settings-panel__group">
        <p>
          configured slicers:{" "}
          <span data-testid="settings-slicer-count">{slicers.length}</span>
        </p>
        {slicers.map((row, index) => {
          const rowId = slicerId(row.name, index);
          return (
            <div
              key={`${row.name}:${index}`}
              className="settings-panel__slicer"
              data-testid={`settings-slicer-row-${rowId}`}
            >
              <input
                type="text"
                value={row.name}
                onChange={(ev) => updateSlicer(index, { name: ev.target.value })}
                data-testid={`settings-slicer-name-${rowId}`}
              />
              <input
                type="text"
                value={row.path}
                onChange={(ev) => updateSlicer(index, { path: ev.target.value })}
                data-testid={`settings-slicer-path-${rowId}`}
              />
              <button
                type="button"
                className="btn btn--line btn--sm"
                onClick={() => removeSlicer(index)}
                data-testid={`settings-slicer-remove-${rowId}`}
              >
                remove
              </button>
            </div>
          );
        })}
        <div className="settings-panel__slicer">
          <input
            type="text"
            value={draftSlicerName}
            onChange={(ev) => setDraftSlicerName(ev.target.value)}
            placeholder="slicer name"
            data-testid="settings-slicer-name"
          />
          <input
            type="text"
            value={draftSlicerPath}
            onChange={(ev) => setDraftSlicerPath(ev.target.value)}
            placeholder="slicer path"
            data-testid="settings-slicer-path"
          />
          <button
            type="button"
            className="btn btn--line btn--sm"
            onClick={addSlicer}
            data-testid="settings-slicer-add"
          >
            add slicer
          </button>
        </div>
      </div>
      <p className="settings-panel__meta">
        recent workspaces:{" "}
        <span data-testid="settings-recent-count">
          {(config.recent_workspaces ?? []).length}
        </span>
      </p>
      <button
        type="button"
        className="btn btn--solid btn--sm"
        onClick={save}
        disabled={saveDisabled}
        data-testid="settings-save"
      >
        save
      </button>
      <p className="settings-panel__status" data-testid="settings-status">
        {saveStatus}
      </p>
    </div>
  );
}

function parseOpacity(value: string, fallback: number | undefined): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback ?? 0.85;
  return Math.min(1, Math.max(0, parsed));
}

function slicerId(name: string, index: number): string {
  const normalized = name.trim().replace(/[^a-zA-Z0-9_-]+/g, "-");
  return normalized || `row-${index}`;
}

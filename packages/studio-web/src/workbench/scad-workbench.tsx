// Scad workbench: glues ScadSplitViewer with the Phase 7 parameter and preset
// panels. Overrides (defines) and presets live in React state; the preview
// pipeline hands overrides straight to PreviewRequest.defines. Source parsing
// is not possible on web (server deny list on `.scad`), so overrides are
// authored manually by the user or loaded from a preset file.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WasmClient } from "../wasm-bridge";
import { decodeFileRead, describeFileReadError } from "../viewers/file-read-decoder";
import { ScadSplitViewer } from "../viewers/scad-split-viewer";
import { ParametersPanel, type ParameterOverride } from "./parameters-panel";
import { PresetsPanel, type PresetEntry } from "./presets-panel";
import {
  derivePresetPath,
  derivePresetPathLabel,
  parsePresetFile,
  stringifyPresetFile,
} from "./preset-io";

type ScadWorkbenchProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  onPreviewStatus?: (status: string) => void;
  refreshSignal: number;
  onLog: (level: "info" | "warn" | "error", message: string) => void;
};

export function ScadWorkbench(props: ScadWorkbenchProps) {
  const { path, client, label, onPreviewStatus, refreshSignal, onLog } = props;

  const [overrides, setOverrides] = useState<ParameterOverride[]>([]);
  const [appliedDefines, setAppliedDefines] = useState<string[]>([]);
  const [previewStatus, setPreviewStatus] = useState<string>("preview pending");

  const presetPath = useMemo(() => derivePresetPath(path), [path]);
  const presetPathLabel = useMemo(() => derivePresetPathLabel(path), [path]);

  const [presets, setPresets] = useState<PresetEntry[]>([]);
  const [presetLoading, setPresetLoading] = useState(false);
  const [presetError, setPresetError] = useState<string | null>(null);

  const bumpPreviewRef = useRef(0);
  const [bumpedPreview, setBumpedPreview] = useState(0);

  const emitStatus = useCallback(
    (status: string) => {
      setPreviewStatus(status);
      onPreviewStatus?.(status);
    },
    [onPreviewStatus],
  );

  const applyOverrides = useCallback(() => {
    const defines = overrides
      .map((o) => `${o.name.trim()}=${o.value}`)
      .filter((s) => s.length > 1);
    setAppliedDefines(defines);
    bumpPreviewRef.current += 1;
    setBumpedPreview(bumpPreviewRef.current);
    onLog("info", `parameters apply (${defines.length} defines)`);
  }, [overrides, onLog]);

  const restoreDefaults = useCallback(() => {
    setOverrides([]);
    setAppliedDefines([]);
    bumpPreviewRef.current += 1;
    setBumpedPreview(bumpPreviewRef.current);
    onLog("info", "parameters restore defaults");
  }, [onLog]);

  const loadPresets = useCallback(() => {
    if (!presetPath) {
      setPresetError("no preset path");
      return;
    }
    setPresetLoading(true);
    setPresetError(null);
    client
      .dispatchFileRead({ path: presetPath })
      .then((response) => {
        const decoded = decodeFileRead(response);
        if (!decoded) {
          throw new Error("unexpected FileRead payload");
        }
        if (decoded.kind !== "utf8") {
          throw new Error("preset file is not utf-8");
        }
        const file = parsePresetFile(decoded.text);
        setPresets(file.presets);
        setPresetLoading(false);
        onLog("info", `presets loaded (${file.presets.length})`);
      })
      .catch((err) => {
        const message = describeFileReadError(err);
        // "file not found" is a normal initial state for a fresh scad file.
        if (/not found/i.test(message)) {
          setPresets([]);
          setPresetLoading(false);
          setPresetError(null);
          onLog("info", "presets file not found, treating as empty");
          return;
        }
        setPresetError(message);
        setPresetLoading(false);
        onLog("warn", `preset load failed: ${message}`);
      });
  }, [client, presetPath, onLog]);

  useEffect(() => {
    loadPresets();
  }, [loadPresets]);

  const persistPresets = useCallback(
    (next: PresetEntry[]) => {
      if (!presetPath) return;
      const payload = stringifyPresetFile({ version: 1, presets: next });
      client
        .dispatchFileWriteText({ path: presetPath, contents: payload })
        .then(() => {
          setPresets(next);
          onLog("info", `presets saved (${next.length})`);
        })
        .catch((err) => {
          const message = describeFileReadError(err);
          setPresetError(message);
          onLog("error", `preset save failed: ${message}`);
        });
    },
    [client, presetPath, onLog],
  );

  const savePreset = useCallback(
    (name: string) => {
      const defines = overrides
        .map((o) => `${o.name.trim()}=${o.value}`)
        .filter((s) => s.length > 1);
      const next = [
        ...presets.filter((item) => item.name !== name),
        { name, defines },
      ];
      persistPresets(next);
    },
    [overrides, presets, persistPresets],
  );

  const deletePreset = useCallback(
    (name: string) => {
      const next = presets.filter((item) => item.name !== name);
      persistPresets(next);
    },
    [presets, persistPresets],
  );

  const loadPreset = useCallback(
    (name: string) => {
      const preset = presets.find((item) => item.name === name);
      if (!preset) return;
      const next: ParameterOverride[] = preset.defines.map((define, index) => {
        const eq = define.indexOf("=");
        const pname = eq >= 0 ? define.slice(0, eq) : define;
        const pvalue = eq >= 0 ? define.slice(eq + 1) : "";
        return { id: `${name}-${index}`, name: pname, value: pvalue };
      });
      setOverrides(next);
      setAppliedDefines(preset.defines);
      bumpPreviewRef.current += 1;
      setBumpedPreview(bumpPreviewRef.current);
      onLog("info", `preset loaded: ${name}`);
    },
    [presets, onLog],
  );

  const handleAddOverride = useCallback((name: string, value: string) => {
    setOverrides((prev) => [
      ...prev,
      { id: `${name}-${Date.now()}-${prev.length}`, name, value },
    ]);
  }, []);

  const handleUpdateOverride = useCallback(
    (id: string, patch: Partial<ParameterOverride>) => {
      setOverrides((prev) =>
        prev.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
      );
    },
    [],
  );

  const handleRemoveOverride = useCallback((id: string) => {
    setOverrides((prev) => prev.filter((entry) => entry.id !== id));
  }, []);

  const splitKey = `${label}:${appliedDefines.join("|")}:${bumpedPreview}:${refreshSignal}`;

  return (
    <div className="scad-workbench" data-testid="scad-workbench">
      <ScadSplitViewer
        key={splitKey}
        path={path}
        client={client}
        label={label}
        defines={appliedDefines}
        onPreviewStatus={emitStatus}
      />
      <div className="scad-workbench__panels">
        <ParametersPanel
          overrides={overrides}
          onAddOverride={handleAddOverride}
          onUpdateOverride={handleUpdateOverride}
          onRemoveOverride={handleRemoveOverride}
          onApply={applyOverrides}
          onRestoreDefaults={restoreDefaults}
          previewStatus={previewStatus}
        />
        <PresetsPanel
          presetPath={presetPathLabel}
          presets={presets}
          loading={presetLoading}
          error={presetError}
          onReload={loadPresets}
          onLoadPreset={loadPreset}
          onSavePreset={savePreset}
          onDeletePreset={deletePreset}
        />
      </div>
    </div>
  );
}

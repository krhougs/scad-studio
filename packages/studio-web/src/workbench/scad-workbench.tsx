import {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import type React from "react";
import type { CameraPreset, CameraState } from "../canvas/camera-state";
import type { AppConfigShape } from "../config/app-config";
import { WasmClient } from "../wasm-bridge";
import {
  decodeFileRead,
  describeFileReadError,
} from "../viewers/file-read-decoder";
import { ScadPreviewViewer } from "../viewers/scad-preview-viewer";
import type { MeshInfo } from "../viewers/mesh-info";
import type { MeshViewerOptions } from "../viewers/viewer-options";
import { ParametersPanel } from "./parameters-panel";
import {
  applyPresetValues,
  currentParameterValues,
  formatCurrentDefines,
  mergeParameterEntries,
  parseParameterSource,
  restoreAllParameterValues,
  restoreParameterValue,
  updateParameterValue,
  type ParameterEntry,
  type ParameterValue,
} from "./parameter-model";
import { PresetsPanel, type PresetEntry } from "./presets-panel";
import {
  deriveLegacyPresetPaths,
  derivePresetPath,
  derivePresetPathLabel,
  parsePresetFile,
  stringifyPresetFile,
} from "./preset-io";

type ScadWorkbenchProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  state: ScadWorkbenchState;
  config: AppConfigShape | null;
  cameraPreset?: CameraPreset | null;
  cameraOverride?: CameraState | null;
  viewerOptions?: MeshViewerOptions;
  refreshSignal: number;
  onMeshInfo?: (info: MeshInfo | null) => void;
  onCameraChange?: (camera: CameraState) => void;
};

export type ScadWorkbenchState = {
  parameterEntries: ParameterEntry[];
  parameterWarnings: string[];
  presets: PresetEntry[];
  presetPathLabel: string;
  presetLoading: boolean;
  presetError: string | null;
  previewStatus: string;
  appliedDefines: string[];
  sourceReady: boolean;
  emitStatus: (status: string) => void;
  restoreDefaults: () => void;
  updateParameter: (name: string, value: ParameterValue) => void;
  restoreParameter: (name: string) => void;
  loadPresets: () => void;
  loadPreset: (name: string) => void;
  savePreset: (name: string) => void;
  deletePreset: (name: string) => void;
};

export function ScadWorkbench(props: ScadWorkbenchProps) {
  const {
    path,
    client,
    label,
    state,
    config,
    cameraPreset,
    cameraOverride,
    viewerOptions,
    refreshSignal,
    onMeshInfo,
    onCameraChange,
  } = props;

  return (
    <div className="scad-workbench" data-testid="scad-workbench">
      <ScadPreviewViewer
        path={path}
        client={client}
        label={label}
        defines={state.appliedDefines}
        configuredOpenscadPath={config?.openscad_path ?? null}
        cameraPreset={cameraPreset}
        cameraOverride={cameraOverride}
        viewerOptions={viewerOptions}
        previewEnabled={state.sourceReady}
        refreshSignal={refreshSignal}
        onPreviewStatus={state.emitStatus}
        onStats={(stats) => {
          if (!stats) onMeshInfo?.(null);
        }}
        onInfo={onMeshInfo}
        onCameraChange={onCameraChange}
      />
    </div>
  );
}

type ScadWorkbenchStateInput = {
  path: unknown | null;
  client: WasmClient | null;
  refreshSignal: number;
  onLog: (level: "info" | "warn" | "error", message: string) => void;
  onPreviewStatus?: (status: string) => void;
  enabled?: boolean;
};

export function useScadWorkbenchState({
  path,
  client,
  refreshSignal,
  onLog,
  onPreviewStatus,
  enabled = true,
}: ScadWorkbenchStateInput): ScadWorkbenchState {
  const active = enabled && client !== null && path !== null;
  const [parameterEntries, setParameterEntries] = useState<ParameterEntry[]>([]);
  const [parameterWarnings, setParameterWarnings] = useState<string[]>([]);
  const [appliedDefines, setAppliedDefines] = useState<string[]>([]);
  const [sourceReady, setSourceReady] = useState(false);
  const [previewStatus, setPreviewStatus] = useState<string>("preview pending");
  const [presets, setPresets] = useState<PresetEntry[]>([]);
  const [presetLoading, setPresetLoading] = useState(false);
  const [presetError, setPresetError] = useState<string | null>(null);

  const presetPath = useMemo(() => derivePresetPath(path), [path]);
  const presetPathLabel = useMemo(() => derivePresetPathLabel(path), [path]);
  const legacyPresetPaths = useMemo(() => deriveLegacyPresetPaths(path), [path]);

  useEffect(() => {
    setParameterEntries([]);
    setParameterWarnings([]);
    setAppliedDefines([]);
    setSourceReady(false);
    setPresets([]);
    setPresetError(null);
  }, [active, path]);

  const emitStatus = useCallback(
    (status: string) => {
      setPreviewStatus(status);
      onPreviewStatus?.(status);
    },
    [onPreviewStatus],
  );

  const restoreDefaults = useCallback(() => {
    const next = restoreAllParameterValues(parameterEntries);
    setParameterEntries(next);
    setAppliedDefines(formatCurrentDefines(next));
    onLog("info", "parameters restore defaults");
  }, [onLog, parameterEntries]);

  const applySourceText = useCallback(
    (source: string | null, ready: boolean) => {
      setSourceReady(ready);
      if (source === null) {
        setParameterEntries([]);
        setParameterWarnings([]);
        setAppliedDefines([]);
        return;
      }
      const parsed = parseParameterSource(source);
      setParameterWarnings(parsed.warnings);
      setParameterEntries((previous) => {
        const next = mergeParameterEntries(previous, parsed.entries);
        setAppliedDefines(formatCurrentDefines(next));
        return next;
      });
      for (const warning of parsed.warnings) {
        onLog("warn", `parameter parse warning: ${warning}`);
      }
    },
    [onLog],
  );

  useEffect(() => {
    if (!active || !client || !path) return;
    let cancelled = false;
    applySourceText(null, false);

    client
      .dispatchFileRead({ path })
      .then((response) => {
        if (cancelled) return;
        const decoded = decodeFileRead(response);
        if (!decoded || decoded.kind !== "utf8") {
          applySourceText(null, true);
          return;
        }
        applySourceText(decoded.text, true);
      })
      .catch((err) => {
        if (cancelled) return;
        onLog("warn", `scad source unavailable: ${describeFileReadError(err)}`);
        applySourceText(null, true);
      });

    return () => {
      cancelled = true;
    };
  }, [active, applySourceText, client, onLog, path, refreshSignal]);

  const handleUpdateParameter = useCallback(
    (name: string, value: ParameterValue) => {
      setParameterEntries((prev) => {
        return updateParameterValue(prev, name, value);
      });
    },
    [],
  );

  const handleRestoreParameter = useCallback((name: string) => {
    setParameterEntries((prev) => {
      return restoreParameterValue(prev, name);
    });
  }, []);

  useEffect(() => {
    if (!sourceReady) return;
    const handle = window.setTimeout(() => {
      const defines = formatCurrentDefines(parameterEntries);
      setAppliedDefines(defines);
      onLog("info", `parameters preview update (${defines.length} defines)`);
    }, 250);
    return () => window.clearTimeout(handle);
  }, [onLog, parameterEntries, sourceReady]);

  const loadPresetsFrom = useCallback(
    async (
      targetPath: unknown,
    ): Promise<{ file: PresetEntry[]; source: "primary" | "legacy" }> => {
      if (!client) throw new Error("transport not ready");
      const response = await client.dispatchFileRead({ path: targetPath });
      const decoded = response as Record<string, unknown>;
      const payload =
        (decoded["payload"] as Record<string, unknown> | undefined) ?? decoded;
      const contents =
        payload["contents"] as Record<string, unknown> | undefined;
      if (!contents || contents["kind"] !== "utf8_text") {
        throw new Error("preset file is not utf-8");
      }
      const text = contents["payload"];
      if (typeof text !== "string") {
        throw new Error("preset file payload missing");
      }
      return {
        file: parsePresetFile(text).presets,
        source: targetPath === presetPath ? "primary" : "legacy",
      };
    },
    [client, presetPath],
  );

  const loadPresets = useCallback(() => {
    if (!active || !client) return;
    if (!presetPath) {
      setPresetError("no preset path");
      return;
    }
    setPresetLoading(true);
    setPresetError(null);
    loadFirstExistingPreset([presetPath, ...legacyPresetPaths], loadPresetsFrom)
      .then(({ file, source }) => {
        setPresets(file);
        setPresetLoading(false);
        onLog(
          "info",
          `presets loaded (${file.length}) from ${source === "primary" ? "scad.json" : "legacy presets.json"}`,
        );
      })
      .catch((err) => {
        const message = describeFileReadError(err);
        if (isMissingPresetError(err)) {
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
  }, [active, client, legacyPresetPaths, loadPresetsFrom, onLog, presetPath]);

  useEffect(() => {
    if (active) loadPresets();
  }, [active, loadPresets, refreshSignal]);

  const persistPresets = useCallback(
    (next: PresetEntry[]) => {
      if (!client || !presetPath) return;
      const payload = stringifyPresetFile({ presets: next });
      client
        .dispatchFileWriteText({ path: presetPath, contents: payload })
        .then(() => {
          setPresets(next);
          setPresetError(null);
          onLog("info", `presets saved (${next.length})`);
        })
        .catch((err) => {
          const message = describeFileReadError(err);
          setPresetError(message);
          onLog("error", `preset save failed: ${message}`);
        });
    },
    [client, onLog, presetPath],
  );

  const savePreset = useCallback(
    (name: string) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      const next = [
        ...presets.filter((item) => item.name !== trimmed),
        { name: trimmed, values: currentParameterValues(parameterEntries) },
      ];
      persistPresets(next);
    },
    [parameterEntries, persistPresets, presets],
  );

  const deletePreset = useCallback(
    (name: string) => {
      const next = presets.filter((item) => item.name !== name);
      persistPresets(next);
    },
    [persistPresets, presets],
  );

  const loadPreset = useCallback(
    (name: string) => {
      const preset = presets.find((item) => item.name === name);
      if (!preset) return;
      const next = applyPresetValues(parameterEntries, preset.values);
      setParameterEntries(next);
      setAppliedDefines(formatCurrentDefines(next));
      onLog("info", `preset loaded: ${name}`);
    },
    [onLog, parameterEntries, presets],
  );

  return useMemo(
    () => ({
      parameterEntries,
      parameterWarnings,
      presets,
      presetPathLabel,
      presetLoading,
      presetError,
      previewStatus,
      appliedDefines,
      sourceReady,
      restoreDefaults,
      updateParameter: handleUpdateParameter,
      restoreParameter: handleRestoreParameter,
      loadPresets,
      loadPreset,
      savePreset,
      deletePreset,
      emitStatus,
    }),
    [
      appliedDefines,
      handleRestoreParameter,
      handleUpdateParameter,
      loadPreset,
      loadPresets,
      parameterEntries,
      parameterWarnings,
      presetError,
      presetLoading,
      presetPathLabel,
      presets,
      restoreDefaults,
      savePreset,
      deletePreset,
      previewStatus,
      sourceReady,
    ],
  );
}

export function scadInspectorPanelsForState(
  state: ScadWorkbenchState,
): { parameters: React.ReactNode; presets: React.ReactNode } {
  return {
    parameters: (
      <ParametersPanel
        entries={state.parameterEntries}
        warnings={state.parameterWarnings}
        onUpdateValue={state.updateParameter}
        onRestoreValue={state.restoreParameter}
        onRestoreDefaults={state.restoreDefaults}
        onSavePreset={state.savePreset}
      />
    ),
    presets: (
      <PresetsPanel
        presetPath={state.presetPathLabel}
        presets={state.presets}
        loading={state.presetLoading}
        error={state.presetError}
        onReload={state.loadPresets}
        onLoadPreset={state.loadPreset}
        onDeletePreset={state.deletePreset}
      />
    ),
  };
}

function isMissingPresetError(err: unknown): boolean {
  return /not found|no such file|cannot find/i.test(describeFileReadError(err));
}

async function loadFirstExistingPreset(
  paths: unknown[],
  load: (
    targetPath: unknown,
  ) => Promise<{ file: PresetEntry[]; source: "primary" | "legacy" }>,
): Promise<{ file: PresetEntry[]; source: "primary" | "legacy" }> {
  let missing: unknown = null;
  for (const targetPath of paths) {
    if (!targetPath) continue;
    try {
      return await load(targetPath);
    } catch (err) {
      if (!isMissingPresetError(err)) throw err;
      missing = err;
    }
  }
  throw missing ?? new Error("preset file not found");
}

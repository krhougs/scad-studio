import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
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
import {
  DEFAULT_PREVIEW_APPEARANCE,
  normalizePreviewAppearance,
  type MeshViewerOptions,
  type PreviewAppearance,
} from "../viewers/viewer-options";
import { ParametersPanel } from "./parameters-panel";
import { PreviewAppearancePanel } from "./preview-appearance-panel";
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
  type PresetFile,
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
  previewAppearance: PreviewAppearance;
  appliedDefines: string[];
  sourceReady: boolean;
  emitStatus: (status: string) => void;
  updatePreviewAppearance: (patch: Partial<PreviewAppearance>) => void;
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
  settingsRefreshSignal: number;
  onLog: (level: "info" | "warn" | "error", message: string) => void;
  onPreviewStatus?: (status: string) => void;
  enabled?: boolean;
};

export function useScadWorkbenchState({
  path,
  client,
  refreshSignal,
  settingsRefreshSignal,
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
  const [previewAppearance, setPreviewAppearance] = useState<PreviewAppearance>({
    ...DEFAULT_PREVIEW_APPEARANCE,
  });
  const presetsRef = useRef(presets);
  const previewAppearanceRef = useRef(previewAppearance);
  const presetsLoadedRef = useRef(false);
  const previewAppearanceDirtyRef = useRef(false);
  const previewAppearanceVersionRef = useRef(0);
  const settingsWriteChainRef = useRef<Promise<void>>(Promise.resolve());
  const settingsWriteEpochRef = useRef(0);
  const flushPendingPresetFileWriteRef = useRef<() => void>(() => {});
  const presetLoadSeqRef = useRef(0);
  const pendingPresetFileWriteRef = useRef<{
    path: unknown;
    file: PresetFile;
    timer: number;
    appearanceVersion: number;
  } | null>(null);

  const presetPath = useMemo(() => derivePresetPath(path), [path]);
  const presetPathLabel = useMemo(() => derivePresetPathLabel(path), [path]);
  const legacyPresetPaths = useMemo(() => deriveLegacyPresetPaths(path), [path]);
  const applyDefines = useCallback((next: string[]) => {
    setAppliedDefines((previous) => {
      const same =
        previous.length === next.length &&
        previous.every((item, index) => item === next[index]);
      return same ? previous : next;
    });
  }, []);

  useEffect(() => {
    presetsRef.current = presets;
  }, [presets]);

  useEffect(() => {
    previewAppearanceRef.current = previewAppearance;
  }, [previewAppearance]);

  useEffect(() => {
    flushPendingPresetFileWriteRef.current = flushPendingPresetFileWrite;
  });

  useEffect(() => {
    return () => {
      flushPendingPresetFileWriteRef.current();
    };
  }, []);

  useEffect(() => {
    presetLoadSeqRef.current += 1;
    flushPendingPresetFileWrite();
    presetsLoadedRef.current = false;
    previewAppearanceDirtyRef.current = false;
    previewAppearanceVersionRef.current += 1;
    setParameterEntries([]);
    setParameterWarnings([]);
    applyDefines([]);
    setSourceReady(false);
    setPresets([]);
    setPresetError(null);
    setPreviewAppearance({ ...DEFAULT_PREVIEW_APPEARANCE });
  }, [active, applyDefines, path]);

  function enqueuePresetFileWrite(
    targetPath: unknown,
    file: PresetFile,
    successMessage: string,
    appearanceVersion: number,
  ): Promise<void> {
    if (!client || !targetPath) return Promise.resolve();
    const writeEpoch = settingsWriteEpochRef.current;
    const task = settingsWriteChainRef.current
      .catch(() => {})
      .then(() => {
        if (writeEpoch !== settingsWriteEpochRef.current) {
          throw staleSettingsWriteError();
        }
        const payload = stringifyPresetFile(file);
        return client.dispatchFileWriteText({ path: targetPath, contents: payload });
      })
      .then(() => {
        if (writeEpoch !== settingsWriteEpochRef.current) {
          throw staleSettingsWriteError();
        }
        if (appearanceVersion === previewAppearanceVersionRef.current) {
          previewAppearanceDirtyRef.current = false;
        }
        onLog("info", successMessage);
      })
      .catch((err) => {
        if (!isStaleSettingsWriteError(err)) {
          settingsWriteEpochRef.current += 1;
        }
        throw err;
      });
    settingsWriteChainRef.current = task.catch(() => {});
    return task;
  }

  function schedulePresetFileWrite(
    targetPath: unknown,
    file: PresetFile,
    successMessage: string,
    appearanceVersion: number,
  ): void {
    if (!client || !targetPath) return;
    if (pendingPresetFileWriteRef.current) {
      window.clearTimeout(pendingPresetFileWriteRef.current.timer);
    }
    const timer = window.setTimeout(() => {
      pendingPresetFileWriteRef.current = null;
      if (
        !previewAppearanceDirtyRef.current ||
        appearanceVersion !== previewAppearanceVersionRef.current
      ) {
        return;
      }
      enqueuePresetFileWrite(
        targetPath,
        file,
        successMessage,
        appearanceVersion,
      ).catch((err) => {
        onLog("error", `scad settings save failed: ${describeFileReadError(err)}`);
      });
    }, 250);
    pendingPresetFileWriteRef.current = {
      path: targetPath,
      file,
      timer,
      appearanceVersion,
    };
  }

  function cancelPendingPresetFileWrite(): void {
    const pending = pendingPresetFileWriteRef.current;
    if (!pending) return;
    window.clearTimeout(pending.timer);
    pendingPresetFileWriteRef.current = null;
  }

  function flushPendingPresetFileWrite(): void {
    const pending = pendingPresetFileWriteRef.current;
    if (!pending) return;
    window.clearTimeout(pending.timer);
    pendingPresetFileWriteRef.current = null;
    enqueuePresetFileWrite(
      pending.path,
      pending.file,
      "preview appearance saved",
      pending.appearanceVersion,
    ).catch((err) => {
      onLog("error", `scad settings save failed: ${describeFileReadError(err)}`);
    });
  }

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
    applyDefines(formatCurrentDefines(next));
    onLog("info", "parameters restore defaults");
  }, [applyDefines, onLog, parameterEntries]);

  const applySourceText = useCallback(
    (source: string | null, ready: boolean) => {
      setSourceReady(ready);
      if (source === null) {
        setParameterEntries([]);
        setParameterWarnings([]);
        applyDefines([]);
        return;
      }
      const parsed = parseParameterSource(source);
      setParameterWarnings(parsed.warnings);
      setParameterEntries((previous) => {
        const next = mergeParameterEntries(previous, parsed.entries);
        applyDefines(formatCurrentDefines(next));
        return next;
      });
      for (const warning of parsed.warnings) {
        onLog("warn", `parameter parse warning: ${warning}`);
      }
    },
    [applyDefines, onLog],
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
      applyDefines(defines);
      onLog("info", `parameters preview update (${defines.length} defines)`);
    }, 250);
    return () => window.clearTimeout(handle);
  }, [applyDefines, onLog, parameterEntries, sourceReady]);

  const loadPresetsFrom = useCallback(
    async (
      targetPath: unknown,
    ): Promise<{
      file: PresetEntry[];
      previewAppearance: PreviewAppearance;
      source: "primary" | "legacy";
    }> => {
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
      const parsed = parsePresetFile(text);
      return {
        file: parsed.presets,
        previewAppearance: parsed.previewAppearance ?? {
          ...DEFAULT_PREVIEW_APPEARANCE,
        },
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
    const loadSeq = ++presetLoadSeqRef.current;
    cancelPendingPresetFileWrite();
    settingsWriteEpochRef.current += 1;
    presetsLoadedRef.current = false;
    setPresetLoading(true);
    setPresetError(null);
    loadFirstExistingPreset([presetPath, ...legacyPresetPaths], loadPresetsFrom)
      .then(({ file, previewAppearance: loadedAppearance, source }) => {
        if (loadSeq !== presetLoadSeqRef.current) return;
        setPresets(file);
        presetsLoadedRef.current = true;
        const nextAppearance = previewAppearanceDirtyRef.current
          ? previewAppearanceRef.current
          : source === "primary"
            ? loadedAppearance
            : { ...DEFAULT_PREVIEW_APPEARANCE };
        setPreviewAppearance(nextAppearance);
        previewAppearanceRef.current = nextAppearance;
        if (previewAppearanceDirtyRef.current) {
          schedulePresetFileWrite(
            presetPath,
            { presets: file, previewAppearance: nextAppearance },
            "preview appearance saved",
            previewAppearanceVersionRef.current,
          );
        }
        setPresetLoading(false);
        onLog(
          "info",
          `presets loaded (${file.length}) from ${source === "primary" ? "scad.json" : "legacy presets.json"}`,
        );
      })
      .catch((err) => {
        if (loadSeq !== presetLoadSeqRef.current) return;
        const message = describeFileReadError(err);
        if (isMissingPresetError(err)) {
          setPresets([]);
          presetsLoadedRef.current = true;
          const nextAppearance = previewAppearanceDirtyRef.current
            ? previewAppearanceRef.current
            : { ...DEFAULT_PREVIEW_APPEARANCE };
          setPreviewAppearance(nextAppearance);
          previewAppearanceRef.current = nextAppearance;
          if (previewAppearanceDirtyRef.current) {
            schedulePresetFileWrite(
              presetPath,
              { presets: [], previewAppearance: nextAppearance },
              "preview appearance saved",
              previewAppearanceVersionRef.current,
            );
          }
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
  }, [active, loadPresets, settingsRefreshSignal]);

  const persistPresets = useCallback(
    (next: PresetEntry[]) => {
      if (!client || !presetPath) return;
      cancelPendingPresetFileWrite();
      const file = {
        presets: next,
        previewAppearance: previewAppearanceRef.current,
      };
      const appearanceVersion = previewAppearanceVersionRef.current;
      const previousPresets = presetsRef.current;
      presetsRef.current = next;
      enqueuePresetFileWrite(
        presetPath,
        file,
        `presets saved (${next.length})`,
        appearanceVersion,
      )
        .then(() => {
          setPresets(next);
          setPresetError(null);
        })
        .catch((err) => {
          presetsRef.current = previousPresets;
          const message = describeFileReadError(err);
          setPresetError(message);
          onLog("error", `preset save failed: ${message}`);
        });
    },
    [client, onLog, presetPath],
  );

  const updatePreviewAppearance = useCallback(
    (patch: Partial<PreviewAppearance>) => {
      setPreviewAppearance((previous) => {
        const next = normalizePreviewAppearance({ ...previous, ...patch });
        previewAppearanceDirtyRef.current = true;
        previewAppearanceVersionRef.current += 1;
        previewAppearanceRef.current = next;
        if (presetsLoadedRef.current) {
          schedulePresetFileWrite(
            presetPath,
            { presets: presetsRef.current, previewAppearance: next },
            "preview appearance saved",
            previewAppearanceVersionRef.current,
          );
        }
        return next;
      });
    },
    [presetPath],
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
      applyDefines(formatCurrentDefines(next));
      onLog("info", `preset loaded: ${name}`);
    },
    [applyDefines, onLog, parameterEntries, presets],
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
      previewAppearance,
      appliedDefines,
      sourceReady,
      updatePreviewAppearance,
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
      previewAppearance,
      updatePreviewAppearance,
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
): {
  appearance: React.ReactNode;
  parameters: React.ReactNode;
  presets: React.ReactNode;
} {
  return {
    appearance: (
      <PreviewAppearancePanel
        appearance={state.previewAppearance}
        onChange={state.updatePreviewAppearance}
      />
    ),
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

function staleSettingsWriteError(): Error {
  return new Error("stale scad settings write skipped");
}

function isStaleSettingsWriteError(err: unknown): boolean {
  return err instanceof Error && err.message === "stale scad settings write skipped";
}

async function loadFirstExistingPreset(
  paths: unknown[],
  load: (
    targetPath: unknown,
  ) => Promise<{
    file: PresetEntry[];
    previewAppearance: PreviewAppearance;
    source: "primary" | "legacy";
  }>,
): Promise<{
  file: PresetEntry[];
  previewAppearance: PreviewAppearance;
  source: "primary" | "legacy";
}> {
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

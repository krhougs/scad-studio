import { useEffect, useMemo, useState } from "react";
import type { CameraPreset, CameraState } from "../canvas/camera-state";
import { CadQueryViewer } from "../viewers/cadquery-viewer";
import type { CadQueryMeshHandleLike } from "../viewers/cadquery-mesh";
import type {
  CadQuerySelectionMode,
  CadQueryViewerMode,
} from "../viewers/cadquery-selection";
import type { MeshInfo } from "../viewers/mesh-info";
import type { MeshViewerOptions } from "../viewers/viewer-options";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";
import { cadQueryPreviewSourcePath } from "./cadquery-source-path";

type CadQuerySourcePreviewProps = {
  sourcePath: unknown;
  client: CadQuerySourcePreviewClient;
  label: string;
  selectionMode: CadQuerySelectionMode;
  mode?: CadQueryViewerMode;
  onMode?: (mode: CadQueryViewerMode) => void;
  viewerOptions?: MeshViewerOptions;
  cameraPreset?: CameraPreset | null;
  cameraOverride?: CameraState | null;
  interactionMode?: "select" | "preview";
  selectionSnapshot?: SelectionUpdateRequest | null;
  refreshSignal?: number;
  onScene?: (scene: import("../viewers/cadquery-mesh").CadQueryScenePayload | null) => void;
  onPreviewStatus?: (status: string) => void;
  onInfo?: (info: MeshInfo | null) => void;
  onCameraChange?: (camera: CameraState) => void;
};

type CadQuerySourcePreviewClient = {
  dispatchCadQueryPreview(params: unknown): Promise<unknown>;
  dispatchCadQueryResultGet(params: unknown): Promise<unknown>;
  takeCadQueryMesh(resultId: string): CadQueryMeshHandleLike | null;
  dispatchSelectionUpdate(params: unknown): Promise<unknown>;
};

type SourcePreviewState =
  | { status: "loading" }
  | { status: "ready"; resultId: string }
  | { status: "error"; message: string };

export function CadQuerySourcePreview(props: CadQuerySourcePreviewProps) {
  const {
    sourcePath,
    client,
    label,
    selectionMode,
    mode,
    onMode,
    viewerOptions,
    cameraPreset,
    cameraOverride,
    interactionMode,
    selectionSnapshot,
    refreshSignal,
    onScene,
    onPreviewStatus,
    onInfo,
    onCameraChange,
  } = props;
  const previewSourcePath = useMemo(
    () => cadQueryPreviewSourcePath(sourcePath, label),
    [label, sourcePath],
  );
  const [state, setState] = useState<SourcePreviewState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    onInfo?.(null);
    onScene?.(null);
    onPreviewStatus?.("cadquery pending");
    setState({ status: "loading" });
    client
      .dispatchCadQueryPreview({
        target_path: previewSourcePath,
        export_formats: [],
        params_json: "{}",
      })
      .then((response) => {
        if (cancelled) return;
        const resultId = cadQueryReadyResultId(response);
        if (!resultId) {
          throw new Error("CadQuery preview response missing result_id");
        }
        setState({ status: "ready", resultId });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const message = errorMessage(err);
        onInfo?.(null);
        onPreviewStatus?.(`cadquery error: ${message}`);
        setState({ status: "error", message });
      });
    return () => {
      cancelled = true;
    };
  }, [client, onInfo, onPreviewStatus, onScene, previewSourcePath, refreshSignal]);

  if (state.status === "ready") {
    return (
      <CadQueryViewer
        resultId={state.resultId}
        client={client}
        label={label}
        selectionMode={selectionMode}
        mode={mode}
        onMode={onMode}
        refreshSignal={refreshSignal}
        cameraPreset={cameraPreset}
        cameraOverride={cameraOverride}
        interactionMode={interactionMode}
        selectionSnapshot={selectionSnapshot}
        viewerOptions={viewerOptions}
        onScene={onScene}
        onPreviewStatus={onPreviewStatus}
        onInfo={onInfo}
        onCameraChange={onCameraChange}
      />
    );
  }

  if (state.status === "error") {
    return (
      <div className="viewer viewer--mesh viewer--cadquery">
        <div className="viewer__error-card" data-testid="cadquery-source-error">
          <strong>CadQuery preview failed</strong>
          <p>{state.message}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="viewer viewer--mesh viewer--cadquery">
      <span className="viewer__loading" data-testid="cadquery-source-loading">
        cadquery pending
      </span>
    </div>
  );
}

export function cadQueryReadyResultId(response: unknown): string | null {
  const record = objectRecord(response);
  const payload = objectRecord(record?.["payload"]);
  const direct = record?.["result_id"];
  const nested = payload?.["result_id"];
  if (typeof nested === "string" && nested.length > 0) return nested;
  if (typeof direct === "string" && direct.length > 0) return direct;
  return null;
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : null;
}

function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

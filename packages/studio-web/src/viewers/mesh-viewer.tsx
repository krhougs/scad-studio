// Mesh viewer: PreviewRequest → PreviewMeshPayload → Three.js WebGL 渲染。
// 所有 wgpu-style 异步在 TS 侧走 WebGL2（同步初始化），wasm 侧只传字节。

import { useEffect, useRef, useState } from "react";
import {
  type CameraPreset,
  type CameraState,
} from "../canvas/camera-state";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "./file-read-decoder";
import {
  createMeshViewer,
  payloadFromPreview,
  type MeshViewerHandle,
} from "./mesh-three";
import type { MeshInfo } from "./mesh-info";
import type { MeshViewerOptions } from "./viewer-options";

type MeshViewerProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  defines?: string[];
  configuredOpenscadPath?: string | null;
  cameraPreset?: CameraPreset | null;
  cameraOverride?: CameraState | null;
  viewerOptions?: MeshViewerOptions;
  previewEnabled?: boolean;
  refreshSignal?: number;
  statusTestId?: string;
  onPreviewStatus?: (status: string) => void;
  onStats?: (stats: { vertices: number; indices: number } | null) => void;
  onInfo?: (info: MeshInfo | null) => void;
  onCameraChange?: (camera: CameraState) => void;
};

type LoadState =
  | { kind: "pending" }
  | { kind: "ready"; vertices: number; indices: number }
  | { kind: "empty" }
  | { kind: "error"; message: string };

export function MeshViewer({
  path,
  client,
  label,
  defines,
  configuredOpenscadPath,
  cameraPreset,
  cameraOverride,
  viewerOptions,
  previewEnabled = true,
  refreshSignal,
  statusTestId,
  onPreviewStatus,
  onStats,
  onInfo,
  onCameraChange,
}: MeshViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const viewerRef = useRef<MeshViewerHandle | null>(null);
  const [state, setState] = useState<LoadState>({ kind: "pending" });
  const lastPathKeyRef = useRef<string | null>(null);
  const hasReadyMeshRef = useRef(false);
  const previewStatusRef = useRef(onPreviewStatus);
  const statsRef = useRef(onStats);
  const infoRef = useRef(onInfo);
  const cameraPresetRef = useRef(cameraPreset);

  useEffect(() => {
    previewStatusRef.current = onPreviewStatus;
    statsRef.current = onStats;
    infoRef.current = onInfo;
    cameraPresetRef.current = cameraPreset;
  }, [cameraPreset, onInfo, onPreviewStatus, onStats]);

  // Viewer lifecycle: create on mount, dispose on unmount.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const viewer = createMeshViewer(canvas);
    viewerRef.current = viewer;
    if (onCameraChange) viewer.onCameraChange(onCameraChange);

    const parent = canvas.parentElement;
    const ro = new ResizeObserver(() => {
      const rect = parent?.getBoundingClientRect();
      if (!rect) return;
      viewer.resize(rect.width, rect.height, window.devicePixelRatio);
    });
    if (parent) ro.observe(parent);
    // initial size
    const rect = parent?.getBoundingClientRect();
    if (rect) viewer.resize(rect.width, rect.height, window.devicePixelRatio);

    return () => {
      ro.disconnect();
      viewer.dispose();
      viewerRef.current = null;
    };
  }, [onCameraChange]);

  // Fetch preview on path change.
  useEffect(() => {
    if (!previewEnabled) {
      setState({ kind: "pending" });
      onPreviewStatus?.("preview pending");
      return;
    }
    let cancelled = false;
    const nextPathKey = stablePathKey(path);
    const samePath = lastPathKeyRef.current === nextPathKey;
    lastPathKeyRef.current = nextPathKey;
    if (!samePath) {
      viewerRef.current?.setMesh(null);
      hasReadyMeshRef.current = false;
      infoRef.current?.(null);
      statsRef.current?.(null);
    }
    setState({ kind: "pending" });
    previewStatusRef.current?.("preview pending");
    client
      .dispatchPreviewRequest({
        source: path,
        defines: defines ?? [],
        kind: "geometry_artifact",
        configured_openscad_path: configuredOpenscadPath ?? null,
      })
      .then((payload) => {
        if (cancelled) return;
        const mesh = payloadFromPreview(payload);
        const viewer = viewerRef.current;
        if (!mesh || mesh.positions.length === 0) {
          if (viewer) viewer.setMesh(null);
          hasReadyMeshRef.current = false;
          setState({ kind: "empty" });
          previewStatusRef.current?.("preview ready (empty)");
          statsRef.current?.(null);
          infoRef.current?.(null);
          return;
        }
        const shouldFrame = !samePath || !hasReadyMeshRef.current;
        const info =
          viewer?.setMesh(mesh, {
            frame: shouldFrame,
            preset: cameraPresetRef.current ?? "iso",
          }) ?? null;
        if (!info) {
          setState({ kind: "empty" });
          previewStatusRef.current?.("preview ready (empty)");
          statsRef.current?.(null);
          infoRef.current?.(null);
          return;
        }
        const vertices = info.vertices;
        const indices = info.indices;
        hasReadyMeshRef.current = true;
        setState({ kind: "ready", vertices, indices });
        previewStatusRef.current?.(
          `preview ready | ${vertices} verts | ${indices} idx`,
        );
        statsRef.current?.({ vertices, indices });
        infoRef.current?.(info);
      })
      .catch((err) => {
        if (cancelled) return;
        const msg = describeFileReadError(err);
        setState({ kind: "error", message: msg });
        previewStatusRef.current?.(`preview error: ${msg}`);
        statsRef.current?.(null);
        infoRef.current?.(null);
        if (!samePath) {
          const viewer = viewerRef.current;
          if (viewer) viewer.setMesh(null);
          hasReadyMeshRef.current = false;
        }
      });
    return () => {
      cancelled = true;
    };
  }, [
    client,
    configuredOpenscadPath,
    defines,
    path,
    previewEnabled,
    refreshSignal,
  ]);

  // Apply preset camera when user picks a view pill. null/undefined = keep
  // the auto-framed camera.
  useEffect(() => {
    if (!cameraPreset) return;
    const viewer = viewerRef.current;
    if (!viewer) return;
    viewer.frameCamera(cameraPreset);
  }, [cameraPreset]);

  useEffect(() => {
    if (!viewerOptions) return;
    viewerRef.current?.setOptions(viewerOptions);
  }, [viewerOptions]);

  useEffect(() => {
    if (!cameraOverride) return;
    viewerRef.current?.setCamera(cameraOverride);
  }, [cameraOverride]);

  return (
    <div
      className="viewer viewer--mesh"
      data-testid="mesh-viewer"
      data-label={label}
    >
      <canvas
        ref={canvasRef}
        className="mesh-viewer__canvas"
        data-testid="mesh-canvas"
      />
      {state.kind === "error" ? (
        <>
          <p
            className="viewer__status-reader"
            data-testid={statusTestId ?? "mesh-status"}
            hidden
          >
            preview error: {state.message}
          </p>
          <div
            className="viewer__error-card"
            data-testid="preview-error-card"
            role="status"
          >
            <div className="viewer__error-card-title">preview error</div>
            <p data-testid="preview-error-message">{state.message}</p>
          </div>
        </>
      ) : (
        <>
          <p
            className="viewer__status-reader"
            data-testid={statusTestId ?? "mesh-status"}
            hidden
          >
            {state.kind === "pending"
              ? "preview pending"
              : state.kind === "empty"
                ? "preview ready (empty mesh)"
                : `preview ready | vertices: ${state.vertices} | indices: ${state.indices}`}
          </p>
          {state.kind === "pending" ? (
            <div className="viewer__loading-overlay" data-testid="mesh-loading-overlay">
              {hasReadyMeshRef.current ? "preview updating…" : "preview loading…"}
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}

function stablePathKey(path: unknown): string {
  try {
    return JSON.stringify(path);
  } catch {
    return String(path);
  }
}

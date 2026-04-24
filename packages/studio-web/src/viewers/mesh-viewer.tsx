// Mesh viewer: PreviewRequest → PreviewMeshPayload → Three.js WebGL 渲染。
// 所有 wgpu-style 异步在 TS 侧走 WebGL2（同步初始化），wasm 侧只传字节。

import { useEffect, useRef, useState } from "react";
import { PRESET_STATES, type CameraPreset } from "../canvas/camera-state";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "./file-read-decoder";
import {
  createMeshViewer,
  payloadFromPreview,
  type MeshViewerHandle,
} from "./mesh-three";
import type { MeshViewerOptions } from "./viewer-options";

type MeshViewerProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  defines?: string[];
  configuredOpenscadPath?: string | null;
  cameraPreset?: CameraPreset | null;
  viewerOptions?: MeshViewerOptions;
  previewEnabled?: boolean;
  refreshSignal?: number;
  statusTestId?: string;
  onPreviewStatus?: (status: string) => void;
  onStats?: (stats: { vertices: number; indices: number } | null) => void;
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
  viewerOptions,
  previewEnabled = true,
  refreshSignal,
  statusTestId,
  onPreviewStatus,
  onStats,
}: MeshViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const viewerRef = useRef<MeshViewerHandle | null>(null);
  const [state, setState] = useState<LoadState>({ kind: "pending" });

  // Viewer lifecycle: create on mount, dispose on unmount.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const viewer = createMeshViewer(canvas);
    viewerRef.current = viewer;

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
  }, []);

  // Fetch preview on path change.
  useEffect(() => {
    if (!previewEnabled) {
      setState({ kind: "pending" });
      onPreviewStatus?.("preview pending");
      return;
    }
    let cancelled = false;
    setState({ kind: "pending" });
    onPreviewStatus?.("preview pending");
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
          setState({ kind: "empty" });
          onPreviewStatus?.("preview ready (empty)");
          onStats?.(null);
          return;
        }
        if (viewer) viewer.setMesh(mesh);
        const vertices = mesh.positions.length / 3;
        const indices = mesh.indices ? mesh.indices.length : vertices;
        setState({ kind: "ready", vertices, indices });
        onPreviewStatus?.(`preview ready | ${vertices} verts | ${indices} idx`);
        onStats?.({ vertices, indices });
      })
      .catch((err) => {
        if (cancelled) return;
        const msg = describeFileReadError(err);
        setState({ kind: "error", message: msg });
        onPreviewStatus?.(`preview error: ${msg}`);
        onStats?.(null);
        const viewer = viewerRef.current;
        if (viewer) viewer.setMesh(null);
      });
    return () => {
      cancelled = true;
    };
  }, [
    client,
    configuredOpenscadPath,
    defines,
    onPreviewStatus,
    onStats,
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
    viewer.setCamera(PRESET_STATES[cameraPreset]);
  }, [cameraPreset]);

  useEffect(() => {
    if (!viewerOptions) return;
    viewerRef.current?.setOptions(viewerOptions);
  }, [viewerOptions]);

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
      )}
    </div>
  );
}

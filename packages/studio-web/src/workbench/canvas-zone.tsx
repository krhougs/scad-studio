// Canvas zone —— Buddin `.canvas` 结构接入真实文档标签：
//   .canvas
//     TabBar
//     .canvas-well
//       .canvas-frame       (预览画布与顶部工具栏)
//       .canvas-statusbar   (固定状态栏，不覆盖预览内容)
//
// 3D 渲染由 viewers/mesh-viewer.tsx 内的 Three.js WebGLRenderer 驱动；
// 没有 wgpu-in-wasm 路径。相机预设按钮直接映射到 mesh viewer 的 setCamera。

import type { CameraPreset, CameraState } from "../canvas/camera-state";
import type { AppConfigShape } from "../config/app-config";
import type { DocumentTab } from "../state/ui-store";
import type { WasmClient } from "../wasm-bridge";
import { ImageViewer } from "../viewers/image-viewer";
import { MarkdownViewer } from "../viewers/markdown-viewer";
import type { MeshInfo } from "../viewers/mesh-info";
import { MeshViewer } from "../viewers/mesh-viewer";
import { meshSceneMetrics } from "../viewers/mesh-render-metrics";
import {
  DEFAULT_MESH_VIEWER_OPTIONS,
  type MeshRenderMode,
  type MeshViewerOptions,
} from "../viewers/viewer-options";
import { useEffect, useRef, useState } from "react";
import { PRESET_STATES } from "../canvas/camera-state";
import { projectViewportGizmoAxes } from "./viewport-gizmo-model";
import { TabBar } from "./tabbar";
import {
  ScadWorkbench,
  type ScadWorkbenchState,
} from "./scad-workbench";

type ViewPreset =
  | "iso"
  | "front"
  | "back"
  | "left"
  | "right"
  | "top"
  | "bottom";
type CanvasZoneProps = {
  phase: string;
  message: string;
  previewTargetLabel: string;
  tabs: DocumentTab[];
  activeTabId: string | null;
  onActivateTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onPreviewStatus: (status: string) => void;
  client: WasmClient | null;
  refreshSignal: number;
  config: AppConfigShape | null;
  meshInfo: MeshInfo | null;
  activeView: ViewPreset;
  onMeshInfo: (info: MeshInfo | null) => void;
  cameraState: CameraState | null;
  cameraOverride: CameraState | null;
  onCameraChange: (camera: CameraState) => void;
  scadWorkbenchState: ScadWorkbenchState;
};

export function CanvasZone(props: CanvasZoneProps) {
  const {
    phase,
    message,
    previewTargetLabel,
    tabs,
    activeTabId,
    onActivateTab,
    onCloseTab,
    onPreviewStatus,
    client,
    refreshSignal,
    config,
    meshInfo,
    activeView,
    onMeshInfo,
    cameraState,
    cameraOverride,
    onCameraChange,
    scadWorkbenchState,
  } = props;

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const isMeshLike = activeTab?.kind === "mesh" || activeTab?.kind === "scad";
  const [viewerOptions, setViewerOptions] = useState<MeshViewerOptions>(
    DEFAULT_MESH_VIEWER_OPTIONS,
  );
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [stageViewport, setStageViewport] = useState({
    width: 0,
    height: 0,
    dpr: 0,
  });
  const metrics = meshSceneMetrics(meshInfo, {
    ...stageViewport,
    projectionMode: viewerOptions.projectionMode,
  });
  const effectiveViewerOptions =
    activeTab?.kind === "scad"
      ? { ...viewerOptions, ...scadWorkbenchState.previewAppearance }
      : viewerOptions;
  const viewportGizmoSize = metrics?.gizmoSize ?? 36;

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const update = () => {
      const rect = stage.getBoundingClientRect();
      setStageViewport({
        width: rect.width,
        height: rect.height,
        dpr: window.devicePixelRatio,
      });
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  const setRenderMode = (renderMode: MeshRenderMode) => {
    setViewerOptions((prev) => ({ ...prev, renderMode }));
  };
  const updateViewerOptions = (patch: Partial<MeshViewerOptions>) => {
    setViewerOptions((prev) => ({ ...prev, ...patch }));
  };

  return (
    <div className="canvas" aria-label="preview canvas" data-testid="workbench-canvas">
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onActivate={onActivateTab}
        onClose={onCloseTab}
      />
      <div className="canvas-well">
        <div className="canvas-frame">
          <div className="canvas-chrome-top">
            {isMeshLike ? (
              <ViewerToolbar
                options={viewerOptions}
                onSetRenderMode={setRenderMode}
                onUpdateOptions={updateViewerOptions}
              />
            ) : (
              <span className="label" data-testid="phase">
                phase · {phase}
              </span>
            )}
          </div>

          <div className="canvas-stage" ref={stageRef}>
            {activeTab ? (
              <ActiveViewer
                tab={activeTab}
                client={client}
                activeView={activeView}
                onPreviewStatus={onPreviewStatus}
                refreshSignal={refreshSignal}
                config={config}
                viewerOptions={effectiveViewerOptions}
                onMeshInfo={onMeshInfo}
                cameraOverride={cameraOverride}
                onCameraChange={onCameraChange}
                scadWorkbenchState={scadWorkbenchState}
              />
            ) : (
              <EmptyStagePlaceholder />
            )}
            {isMeshLike ? (
              <ViewportGizmo
                activeView={activeView}
                camera={cameraState}
                size={viewportGizmoSize}
              />
            ) : null}
          </div>
        </div>

        <div className="canvas-statusbar" data-testid="canvas-statusbar">
          <div className="part-meta" data-testid="canvas-status">
            <div>
              <b>{activeTab ? activeTab.label : previewTargetLabel}</b>
            </div>
            <div className="div" aria-hidden="true" />
            <div className="cell">
              status
              <span data-testid="message" className={messageClass(phase)}>
                {message || "—"}
              </span>
            </div>
            {meshInfo ? (
              <>
                <div className="div" aria-hidden="true" />
                <div className="cell">
                  verts<span>{meshInfo.vertices}</span>
                </div>
                <div className="cell">
                  idx<span>{meshInfo.indices}</span>
                </div>
              </>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}

type ActiveViewerProps = {
  tab: DocumentTab;
  client: WasmClient | null;
  activeView: ViewPreset;
  onPreviewStatus: (status: string) => void;
  refreshSignal: number;
  config: AppConfigShape | null;
  viewerOptions: MeshViewerOptions;
  onMeshInfo: (info: MeshInfo | null) => void;
  cameraOverride: CameraState | null;
  onCameraChange: (camera: CameraState) => void;
  scadWorkbenchState: ScadWorkbenchState;
};

function ActiveViewer({
  tab,
  client,
  activeView,
  onPreviewStatus,
  refreshSignal,
  config,
  viewerOptions,
  onMeshInfo,
  cameraOverride,
  onCameraChange,
  scadWorkbenchState,
}: ActiveViewerProps) {
  useEffect(() => {
    if (tab.kind !== "mesh" && tab.kind !== "scad") {
      onMeshInfo(null);
    }
  }, [onMeshInfo, tab.kind]);

  if (!client) {
    return (
      <p className="viewer__loading" data-testid="viewer-waiting">
        transport not ready
      </p>
    );
  }
  if (tab.kind === "markdown") {
    return (
      <MarkdownViewer
        path={tab.path}
        client={client}
        refreshSignal={refreshSignal}
      />
    );
  }
  if (tab.kind === "image") {
    return (
      <ImageViewer
        path={tab.path}
        client={client}
        refreshSignal={refreshSignal}
      />
    );
  }
  if (tab.kind === "scad") {
    return (
      <ScadWorkbench
        key={tab.id}
        path={tab.path}
        client={client}
        label={tab.label}
        state={scadWorkbenchState}
        config={config}
        cameraPreset={viewPresetToCamera(activeView)}
        cameraOverride={cameraOverride}
        viewerOptions={viewerOptions}
        refreshSignal={refreshSignal}
        onMeshInfo={onMeshInfo}
        onCameraChange={onCameraChange}
      />
    );
  }
  return (
    <MeshViewer
      path={tab.path}
      client={client}
      label={tab.label}
      refreshSignal={refreshSignal}
      cameraPreset={viewPresetToCamera(activeView)}
      cameraOverride={cameraOverride}
      viewerOptions={viewerOptions}
      onPreviewStatus={onPreviewStatus}
      onStats={(stats) => {
        if (!stats) onMeshInfo(null);
      }}
      onInfo={onMeshInfo}
      onCameraChange={onCameraChange}
    />
  );
}

function viewPresetToCamera(view: ViewPreset): CameraPreset {
  switch (view) {
    case "iso":
      return "iso";
    case "front":
      return "front";
    case "back":
      return "back";
    case "left":
      return "left";
    case "top":
      return "top";
    case "right":
      return "right";
    case "bottom":
      return "bottom";
  }
}

function ViewportGizmo({
  activeView,
  camera,
  size,
}: {
  activeView: ViewPreset;
  camera: CameraState | null;
  size: number;
}) {
  const axes = projectViewportGizmoAxes(
    camera ?? PRESET_STATES[viewPresetToCamera(activeView)],
    size,
  );
  return (
    <div
      className="viewport-gizmo"
      data-testid="viewport-gizmo"
      aria-label="viewport gizmo"
    >
      <svg
        className="viewport-gizmo__axes"
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        aria-hidden="true"
      >
        {axes.map((axis) => (
          <g
            key={axis.id}
            data-testid={`viewport-gizmo-axis-${axis.id}`}
            data-end={`${axis.end[0].toFixed(3)},${axis.end[1].toFixed(3)}`}
          >
            <line
              x1={axis.start[0]}
              y1={axis.start[1]}
              x2={axis.end[0]}
              y2={axis.end[1]}
              stroke={axis.color}
              strokeWidth="2.5"
              strokeLinecap="round"
            />
            <circle cx={axis.end[0]} cy={axis.end[1]} r="2.8" fill={axis.color} />
          </g>
        ))}
      </svg>
    </div>
  );
}

function EmptyStagePlaceholder() {
  return (
    <div className="viewer__empty" data-testid="viewer-empty">
      <div className="label">no document open</div>
      <p>pick a file from the files panel to preview.</p>
    </div>
  );
}

function ViewerToolbar({
  options,
  onSetRenderMode,
  onUpdateOptions,
}: {
  options: MeshViewerOptions;
  onSetRenderMode: (mode: MeshRenderMode) => void;
  onUpdateOptions: (patch: Partial<MeshViewerOptions>) => void;
}) {
  return (
    <div className="viewer-toolbar" data-testid="viewer-toolbar">
      <div className="viewer-toolbar__group" aria-label="render mode">
        {(["solid", "wireframe", "xray"] as const).map((mode) => (
          <button
            key={mode}
            type="button"
            className={options.renderMode === mode ? "active" : undefined}
            onClick={() => onSetRenderMode(mode)}
            data-testid={`viewer-render-${mode}`}
          >
            {mode}
          </button>
        ))}
      </div>
      <div className="viewer-toolbar__group" aria-label="color mode">
        {(["color", "mono"] as const).map((mode) => (
          <button
            key={mode}
            type="button"
            className={options.colorMode === mode ? "active" : undefined}
            onClick={() => onUpdateOptions({ colorMode: mode })}
            data-testid={`viewer-color-${mode}`}
          >
            {mode}
          </button>
        ))}
      </div>
      <div className="viewer-toolbar__group" aria-label="projection">
        <button
          type="button"
          className={options.projectionMode === "perspective" ? "active" : undefined}
          onClick={() => onUpdateOptions({ projectionMode: "perspective" })}
          data-testid="viewer-projection-perspective"
        >
          persp
        </button>
        <button
          type="button"
          className={options.projectionMode === "orthographic" ? "active" : undefined}
          onClick={() => onUpdateOptions({ projectionMode: "orthographic" })}
          data-testid="viewer-projection-orthographic"
        >
          ortho
        </button>
      </div>
      <div className="viewer-toolbar__group" aria-label="visibility">
        <ViewerToggle
          active={options.showGrid}
          label="grid"
          testId="viewer-toggle-grid"
          onClick={() => onUpdateOptions({ showGrid: !options.showGrid })}
        />
        <ViewerToggle
          active={options.showAxis}
          label="axis"
          testId="viewer-toggle-axis"
          onClick={() => onUpdateOptions({ showAxis: !options.showAxis })}
        />
        <ViewerToggle
          active={options.showBuildPlate}
          label="plate"
          testId="viewer-toggle-build-plate"
          onClick={() =>
            onUpdateOptions({ showBuildPlate: !options.showBuildPlate })
          }
        />
        <ViewerToggle
          active={options.shadowsEnabled}
          label="shadow"
          testId="viewer-toggle-shadow"
          onClick={() =>
            onUpdateOptions({ shadowsEnabled: !options.shadowsEnabled })
          }
        />
        <ViewerToggle
          active={options.fogEnabled}
          label="fog"
          testId="viewer-toggle-fog"
          onClick={() => onUpdateOptions({ fogEnabled: !options.fogEnabled })}
        />
        <ViewerToggle
          active={options.clipPlaneEnabled}
          label="clip"
          testId="viewer-toggle-clip"
          onClick={() =>
            onUpdateOptions({ clipPlaneEnabled: !options.clipPlaneEnabled })
          }
        />
      </div>
    </div>
  );
}

function ViewerToggle({
  active,
  label,
  testId,
  onClick,
}: {
  active: boolean;
  label: string;
  testId: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={active ? "active" : undefined}
      onClick={onClick}
      data-testid={testId}
      aria-pressed={active}
    >
      {label}
    </button>
  );
}

function messageClass(phase: string): string | undefined {
  if (phase.includes("error")) return "is-err";
  if (phase === "preview-ready") return "is-ok";
  return undefined;
}

export type { ViewPreset };
export type { CameraPreset };

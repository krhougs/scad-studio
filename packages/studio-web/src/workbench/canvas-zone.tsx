// Canvas zone —— Buddin `.canvas` 结构：
//   .canvas
//     TabBar
//     .canvas-well
//       .canvas-chrome-top  (.view-pills + .canvas-info)
//       .canvas-stage       (激活 viewer 或 placeholder)
//       .canvas-chrome-bot  (.part-meta + .canvas-actions)
//
// 3D 渲染由 viewers/mesh-viewer.tsx 内的 Three.js WebGLRenderer 驱动；
// 没有 wgpu-in-wasm 路径。相机预设按钮直接映射到 mesh viewer 的 setCamera。

import type { CameraPreset } from "../canvas/camera-state";
import type { DocumentTab } from "../state/ui-store";
import type { WasmClient } from "../wasm-bridge";
import { ImageViewer } from "../viewers/image-viewer";
import { MarkdownViewer } from "../viewers/markdown-viewer";
import { MeshViewer } from "../viewers/mesh-viewer";
import { TabBar } from "./tabbar";
import { ScadWorkbench } from "./scad-workbench";

type ViewPreset = "iso" | "front" | "top" | "right";
const VIEW_PILLS: { id: ViewPreset; label: string }[] = [
  { id: "iso", label: "iso" },
  { id: "front", label: "front" },
  { id: "top", label: "top" },
  { id: "right", label: "right" },
];

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
  onLog: (level: "info" | "warn" | "error", message: string) => void;
  meshStats: { vertices: number; indices: number } | null;
  activeView: ViewPreset;
  onSelectView: (id: ViewPreset) => void;
  onMeshStats: (stats: { vertices: number; indices: number } | null) => void;
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
    onLog,
    meshStats,
    activeView,
    onSelectView,
    onMeshStats,
  } = props;

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const isMeshLike = activeTab?.kind === "mesh" || activeTab?.kind === "scad";

  return (
    <div className="canvas" aria-label="preview canvas" data-testid="workbench-canvas">
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onActivate={onActivateTab}
        onClose={onCloseTab}
      />
      <div className="canvas-well">
        <div className="canvas-chrome-top">
          {isMeshLike ? (
            <div className="view-pills" data-testid="canvas-view-pills">
              {VIEW_PILLS.map((pill) => (
                <button
                  key={pill.id}
                  type="button"
                  className={pill.id === activeView ? "active" : undefined}
                  onClick={() => onSelectView(pill.id)}
                  data-testid={`view-pill-${pill.id}`}
                >
                  {pill.label}
                </button>
              ))}
            </div>
          ) : (
            <span className="label" data-testid="phase">
              phase · {phase}
            </span>
          )}
          <div className="canvas-info" data-testid="canvas-info">
            <div>{isMeshLike ? activeView : "no preview"}</div>
            <div>units mm</div>
          </div>
        </div>

        <div className="canvas-stage">
          {activeTab ? (
            <ActiveViewer
              tab={activeTab}
              client={client}
              activeView={activeView}
              onPreviewStatus={onPreviewStatus}
              refreshSignal={refreshSignal}
              onLog={onLog}
              onMeshStats={onMeshStats}
            />
          ) : (
            <EmptyStagePlaceholder />
          )}
        </div>

        <div className="canvas-chrome-bot">
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
            {meshStats ? (
              <>
                <div className="div" aria-hidden="true" />
                <div className="cell">
                  verts<span>{meshStats.vertices}</span>
                </div>
                <div className="cell">
                  idx<span>{meshStats.indices}</span>
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
  onLog: (level: "info" | "warn" | "error", message: string) => void;
  onMeshStats: (stats: { vertices: number; indices: number } | null) => void;
};

function ActiveViewer({
  tab,
  client,
  activeView,
  onPreviewStatus,
  refreshSignal,
  onLog,
  onMeshStats,
}: ActiveViewerProps) {
  if (!client) {
    return (
      <p className="viewer__loading" data-testid="viewer-waiting">
        transport not ready
      </p>
    );
  }
  if (tab.kind === "markdown") {
    return <MarkdownViewer path={tab.path} client={client} />;
  }
  if (tab.kind === "image") {
    return <ImageViewer path={tab.path} client={client} />;
  }
  if (tab.kind === "scad") {
    return (
      <ScadWorkbench
        path={tab.path}
        client={client}
        label={tab.label}
        onPreviewStatus={onPreviewStatus}
        refreshSignal={refreshSignal}
        onLog={onLog}
      />
    );
  }
  return (
    <MeshViewer
      path={tab.path}
      client={client}
      label={tab.label}
      cameraPreset={viewPresetToCamera(activeView)}
      onPreviewStatus={onPreviewStatus}
      onStats={onMeshStats}
    />
  );
}

function viewPresetToCamera(view: ViewPreset): CameraPreset {
  // The topbar only surfaces 4 presets; map them onto the 7 camera-state
  // presets Three.js knows about.
  switch (view) {
    case "iso":
      return "iso";
    case "front":
      return "front";
    case "top":
      return "top";
    case "right":
      return "right";
  }
}

function EmptyStagePlaceholder() {
  return (
    <div className="viewer__empty" data-testid="viewer-empty">
      <div className="label">no document open</div>
      <p>pick a file from the inspector tree to preview.</p>
    </div>
  );
}

function messageClass(phase: string): string | undefined {
  if (phase.includes("error")) return "is-err";
  if (phase === "preview-ready") return "is-ok";
  return undefined;
}

export type { ViewPreset };
export type { CameraPreset };

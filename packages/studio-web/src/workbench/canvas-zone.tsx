// Canvas zone: tab bar + viewer container + camera toolbar/statusbar +
// per-tab panels (parameters / presets / slicers / export). The zone owns
// camera state through useCameraController; preview state itself still lives
// inside individual viewer components.

import { useEffect, useRef } from "react";
import { useCameraController } from "../canvas/use-camera-controller";
import { useCanvasRendererController } from "../canvas/renderer-controller";
import type { DocumentTab } from "../state/ui-store";
import type { WasmClient } from "../wasm-bridge";
import { ImageViewer } from "../viewers/image-viewer";
import { MarkdownViewer } from "../viewers/markdown-viewer";
import { MeshViewer } from "../viewers/mesh-viewer";
import { TabBar } from "./tabbar";
import { CanvasToolbar } from "./canvas-toolbar";
import { CanvasStatusbar } from "./canvas-statusbar";
import { ExportPanel } from "./export-panel";
import { SlicerPanel } from "./slicer-panel";
import { ScadWorkbench } from "./scad-workbench";
import { pathLabel } from "./path-utils";

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
  } = props;

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderer = useCanvasRendererController(canvasRef);
  const camera = useCameraController();
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;

  useEffect(() => {
    // Re-publish camera state to the renderer whenever it changes; stub
    // backend no-ops, but real backend picks it up unchanged.
    renderer.setCameraState(camera.camera);
  }, [renderer, camera.camera]);

  const showCanvasChrome = activeTab && (activeTab.kind === "mesh" || activeTab.kind === "scad");
  const rendererStatus = renderer.ready
    ? "renderer ready"
    : `renderer stub (${renderer.lastError ?? "pending"})`;

  return (
    <section
      className="workbench__canvas"
      aria-label="preview canvas"
      data-testid="workbench-canvas"
    >
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onActivate={onActivateTab}
        onClose={onCloseTab}
      />
      {showCanvasChrome ? (
        <CanvasToolbar
          activePreset={camera.activePreset}
          onApplyPreset={camera.applyPreset}
          onReset={camera.reset}
          onZoomIn={camera.zoomIn}
          onZoomOut={camera.zoomOut}
        />
      ) : null}
      {activeTab ? (
        <ActiveViewerSlot
          tab={activeTab}
          client={client}
          onPreviewStatus={onPreviewStatus}
          pointerTargetRef={camera.pointerTargetRef}
          refreshSignal={refreshSignal}
          onLog={onLog}
        />
      ) : (
        <PlaceholderCanvas
          canvasRef={canvasRef}
          rendererStatus={rendererStatus}
          phase={phase}
        />
      )}
      {showCanvasChrome ? (
        <CanvasStatusbar camera={camera.camera} activePreset={camera.activePreset} />
      ) : null}
      <div className="canvas__chrome-bot">
        <div className="canvas__status" data-testid="canvas-status">
          <b>{activeTab ? activeTab.label : previewTargetLabel}</b>
          <div className="canvas__divider" aria-hidden="true" />
          <div className="canvas__cell">
            status
            <span data-testid="message" className={messageClass(phase)}>
              {message || "—"}
            </span>
          </div>
        </div>
      </div>
      {activeTab && activeTab.kind === "mesh" && client ? (
        <div className="canvas__side-panels" data-testid="canvas-mesh-panels">
          <ExportPanel
            client={client}
            source={activeTab.path}
            defaultFilename={defaultExportFilename(activeTab)}
            onStatus={onPreviewStatus}
          />
          <SlicerPanel client={client} />
        </div>
      ) : null}
    </section>
  );
}

type ActiveViewerProps = {
  tab: DocumentTab;
  client: WasmClient | null;
  onPreviewStatus: (status: string) => void;
  pointerTargetRef: (el: HTMLElement | null) => void;
  refreshSignal: number;
  onLog: (level: "info" | "warn" | "error", message: string) => void;
};

function ActiveViewerSlot({
  tab,
  client,
  onPreviewStatus,
  pointerTargetRef,
  refreshSignal,
  onLog,
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
      <div
        ref={pointerTargetRef}
        className="viewer__pointer-stage"
        data-testid="viewer-pointer-stage"
      >
        <ScadWorkbench
          path={tab.path}
          client={client}
          label={tab.label}
          onPreviewStatus={onPreviewStatus}
          refreshSignal={refreshSignal}
          onLog={onLog}
        />
      </div>
    );
  }
  return (
    <div
      ref={pointerTargetRef}
      className="viewer__pointer-stage"
      data-testid="viewer-pointer-stage"
    >
      <MeshViewer
        path={tab.path}
        client={client}
        label={tab.label}
        onPreviewStatus={onPreviewStatus}
      />
    </div>
  );
}

type PlaceholderCanvasProps = {
  canvasRef: React.MutableRefObject<HTMLCanvasElement | null>;
  rendererStatus: string;
  phase: string;
};

function PlaceholderCanvas({
  canvasRef,
  rendererStatus,
  phase,
}: PlaceholderCanvasProps) {
  const attach = (el: HTMLCanvasElement | null) => {
    canvasRef.current = el;
  };
  return (
    <>
      <div className="canvas__grid" aria-hidden="true" />
      <div className="canvas__chrome-top">
        <span className="label" data-testid="phase">
          phase · {phase}
        </span>
        <div className="canvas__info" data-testid="renderer-status">
          <div>{rendererStatus}</div>
          <div>units mm</div>
        </div>
      </div>
      <div className="canvas__stage">
        <canvas
          ref={attach}
          className="canvas__surface"
          width={720}
          height={420}
          data-testid="preview-canvas"
        />
      </div>
    </>
  );
}

function messageClass(phase: string): string | undefined {
  if (phase.includes("error")) return "is-err";
  if (phase === "preview-ready") return "is-ok";
  return undefined;
}

function defaultExportFilename(tab: DocumentTab): string {
  const label = pathLabel(tab.path) || tab.label;
  if (!label) return "export.stl";
  const idx = label.lastIndexOf(".");
  const stem = idx >= 0 ? label.slice(0, idx) : label;
  return `${stem || "export"}.stl`;
}

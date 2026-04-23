// Canvas zone: tab bar + active viewer container + bottom status strip.
// Phase 6 change: the zone hosts whatever viewer corresponds to the active
// DocumentTab. When no tab is open, the placeholder canvas (Phase 4) is
// rendered instead so the handshake flow still has something to show.

import { useRef } from "react";
import { useCanvasRendererController } from "../canvas/renderer-controller";
import type { DocumentTab } from "../state/ui-store";
import type { WasmClient } from "../wasm-bridge";
import { ImageViewer } from "../viewers/image-viewer";
import { MarkdownViewer } from "../viewers/markdown-viewer";
import { MeshViewer } from "../viewers/mesh-viewer";
import { ScadSplitViewer } from "../viewers/scad-split-viewer";
import { TabBar } from "./tabbar";

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
};

export function CanvasZone({
  phase,
  message,
  previewTargetLabel,
  tabs,
  activeTabId,
  onActivateTab,
  onCloseTab,
  onPreviewStatus,
  client,
}: CanvasZoneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderer = useCanvasRendererController(canvasRef);
  const activeTab =
    tabs.find((tab) => tab.id === activeTabId) ?? null;

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
      {activeTab ? (
        <ActiveViewer
          tab={activeTab}
          client={client}
          onPreviewStatus={onPreviewStatus}
        />
      ) : (
        <PlaceholderCanvas
          canvasRef={canvasRef}
          rendererStatus={rendererStatus}
          phase={phase}
        />
      )}
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
    </section>
  );
}

type ActiveViewerProps = {
  tab: DocumentTab;
  client: WasmClient | null;
  onPreviewStatus: (status: string) => void;
};

function ActiveViewer({ tab, client, onPreviewStatus }: ActiveViewerProps) {
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
      <ScadSplitViewer
        path={tab.path}
        client={client}
        label={tab.label}
        onPreviewStatus={onPreviewStatus}
      />
    );
  }
  return (
    <MeshViewer
      path={tab.path}
      client={client}
      label={tab.label}
      onPreviewStatus={onPreviewStatus}
    />
  );
}

type PlaceholderCanvasProps = {
  canvasRef: React.RefObject<HTMLCanvasElement>;
  rendererStatus: string;
  phase: string;
};

function PlaceholderCanvas({
  canvasRef,
  rendererStatus,
  phase,
}: PlaceholderCanvasProps) {
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
          ref={canvasRef}
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

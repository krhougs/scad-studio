// Canvas zone: preview surface + floating chrome.
// canvas 本身通过 CanvasRendererController 挂载到 wasm renderer（Phase 2 stub
// 下 ready=false，保持状态文字可读）。

import { useRef } from "react";
import { useCanvasRendererController } from "../canvas/renderer-controller";

type CanvasZoneProps = {
  phase: string;
  message: string;
  previewTargetLabel: string;
};

export function CanvasZone({ phase, message, previewTargetLabel }: CanvasZoneProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderer = useCanvasRendererController(canvasRef);

  const rendererStatus = renderer.ready
    ? "renderer ready"
    : `renderer stub (${renderer.lastError ?? "pending"})`;

  return (
    <section
      className="workbench__canvas"
      aria-label="preview canvas"
      data-testid="workbench-canvas"
    >
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
      <div className="canvas__chrome-bot">
        <div className="canvas__status" data-testid="canvas-status">
          <b>{previewTargetLabel}</b>
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

function messageClass(phase: string): string | undefined {
  if (phase.includes("error")) return "is-err";
  if (phase === "preview-ready") return "is-ok";
  return undefined;
}

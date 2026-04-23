// StrictMode double-mount test for useCanvasRendererController.
// renderer_create is a Phase 2b stub that always returns Err; this test asserts
// the hook captures that error without crashing the component tree and that
// repeated mount/unmount cycles stay idempotent.

import { StrictMode, useRef } from "react";
import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useCanvasRendererController } from "../../src/canvas/renderer-controller";

type Probe = {
  lastError: string | null;
  ready: boolean;
  resize: (w: number, h: number, dpr: number) => void;
};

let latestProbe: Probe | null = null;

function Harness({ onProbe }: { onProbe: (probe: Probe) => void }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const controller = useCanvasRendererController(canvasRef);
  onProbe(controller);
  return <canvas ref={canvasRef} width={320} height={180} />;
}

describe("useCanvasRendererController", () => {
  afterEach(() => {
    latestProbe = null;
  });

  it("surfaces renderer_create stub error without throwing under StrictMode", () => {
    const { unmount } = render(
      <StrictMode>
        <Harness onProbe={(probe) => (latestProbe = probe)} />
      </StrictMode>,
    );
    expect(latestProbe).not.toBeNull();
    expect(latestProbe?.ready).toBe(false);
    expect(latestProbe?.lastError).toMatch(/renderer/i);
    expect(() => latestProbe?.resize(800, 600, 2)).not.toThrow();
    expect(() => unmount()).not.toThrow();
  });

  it("can remount after unmount without throwing", () => {
    const first = render(
      <StrictMode>
        <Harness onProbe={(probe) => (latestProbe = probe)} />
      </StrictMode>,
    );
    act(() => {
      first.unmount();
    });
    const second = render(
      <StrictMode>
        <Harness onProbe={(probe) => (latestProbe = probe)} />
      </StrictMode>,
    );
    expect(latestProbe).not.toBeNull();
    expect(latestProbe?.lastError).toMatch(/renderer/i);
    act(() => {
      second.unmount();
    });
  });
});

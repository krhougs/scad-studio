// React hook binding a canvas DOM ref to the wasm renderer handle.
// The Phase 2b stub always returns Err from renderer_create, so the hook must
// stay resilient and surface the error without crashing the React tree.

import { useEffect, useRef, useState } from "react";
import * as Wasm from "@scad-studio/studio-web-wasm";

export type RendererControllerState = {
  ready: boolean;
  lastError: string | null;
};

export type RendererController = RendererControllerState & {
  resize: (width: number, height: number, devicePixelRatio: number) => void;
};

type Handle = Wasm.RendererHandle | null;

export function useCanvasRendererController(
  canvasRef: React.RefObject<HTMLCanvasElement | null>,
): RendererController {
  const handleRef = useRef<Handle>(null);
  const [state, setState] = useState<RendererControllerState>({
    ready: false,
    lastError: null,
  });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    if (!canvas.id) {
      canvas.id = `scad-canvas-${Math.random().toString(36).slice(2)}`;
    }

    let cancelled = false;
    try {
      const handle = Wasm.renderer_create(canvas.id);
      if (cancelled) {
        Wasm.renderer_destroy(handle);
        return;
      }
      handleRef.current = handle;
      setState({ ready: true, lastError: null });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setState({ ready: false, lastError: message });
    }

    return () => {
      cancelled = true;
      const handle = handleRef.current;
      handleRef.current = null;
      if (handle) {
        try {
          Wasm.renderer_destroy(handle);
        } catch {
          // destroy must be idempotent; swallow errors during teardown.
        }
      }
    };
  }, [canvasRef]);

  const resize = (width: number, height: number, devicePixelRatio: number) => {
    const handle = handleRef.current;
    if (!handle) return;
    try {
      Wasm.renderer_resize(handle, width, height, devicePixelRatio);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setState((prev) => ({ ...prev, lastError: message }));
    }
  };

  return {
    ready: state.ready,
    lastError: state.lastError,
    resize,
  };
}

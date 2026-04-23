// useCameraController: binds pointer and wheel events on a target element to
// the pure camera helpers. State is owned by the hook; the caller gets the
// current camera, the active preset (null = custom), and a handful of imperative
// methods for the canvas toolbar.

import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyPreset as applyPresetPure,
  classifyPointerMode,
  orbitBy,
  panBy,
  resetCamera,
  zoomBy,
} from "./camera-controls";
import {
  CAMERA_PRESETS,
  CameraPreset,
  CameraState,
  defaultCameraState,
  PRESET_STATES,
} from "./camera-state";

export type UseCameraController = {
  camera: CameraState;
  activePreset: CameraPreset | null;
  applyPreset: (preset: CameraPreset) => void;
  reset: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
  pointerTargetRef: (element: HTMLElement | null) => void;
};

function computeActivePreset(state: CameraState): CameraPreset | null {
  for (const preset of CAMERA_PRESETS) {
    const ref = PRESET_STATES[preset];
    const dx = Math.abs(state.position[0] - ref.position[0]);
    const dy = Math.abs(state.position[1] - ref.position[1]);
    const dz = Math.abs(state.position[2] - ref.position[2]);
    if (dx < 1e-3 && dy < 1e-3 && dz < 1e-3) {
      return preset;
    }
  }
  return null;
}

export function useCameraController(): UseCameraController {
  const [camera, setCamera] = useState<CameraState>(() => defaultCameraState());
  const [activePreset, setActivePreset] = useState<CameraPreset | null>("iso");
  const cameraRef = useRef<CameraState>(camera);
  cameraRef.current = camera;

  const [element, setElement] = useState<HTMLElement | null>(null);

  const commit = useCallback(
    (next: CameraState, preset: CameraPreset | null) => {
      setCamera(next);
      setActivePreset(preset);
    },
    [],
  );

  const applyPreset = useCallback(
    (preset: CameraPreset) => {
      commit(applyPresetPure(preset), preset);
    },
    [commit],
  );

  const reset = useCallback(() => {
    const next = resetCamera();
    commit(next, computeActivePreset(next));
  }, [commit]);

  const zoomIn = useCallback(() => {
    const next = zoomBy(cameraRef.current, -0.2);
    commit(next, computeActivePreset(next));
  }, [commit]);

  const zoomOut = useCallback(() => {
    const next = zoomBy(cameraRef.current, 0.2);
    commit(next, computeActivePreset(next));
  }, [commit]);

  useEffect(() => {
    if (!element) return;
    return bindPointerHandlers({ element, cameraRef, commit });
  }, [element, commit]);

  return {
    camera,
    activePreset,
    applyPreset,
    reset,
    zoomIn,
    zoomOut,
    pointerTargetRef: setElement,
  };
}

type BindDeps = {
  element: HTMLElement;
  cameraRef: React.MutableRefObject<CameraState>;
  commit: (next: CameraState, preset: CameraPreset | null) => void;
};

function bindPointerHandlers(deps: BindDeps): () => void {
  const { element, cameraRef, commit } = deps;
  let pointer: null | {
    mode: "orbit" | "pan";
    pointerId: number;
    lastX: number;
    lastY: number;
  } = null;
  const onPointerDown = (ev: PointerEvent) => {
    // Only start an orbit/pan when the pointer lands on the stage itself;
    // nested interactive elements (buttons, inputs) must keep their native
    // click semantics. Without this guard pointerdown on a child bubbles up,
    // we setPointerCapture on the stage, and the subsequent mouseup/click is
    // consumed by the capture target instead of the button.
    const target = ev.target as HTMLElement | null;
    if (target && target !== element) {
      if (target.closest("button, input, select, textarea, a")) return;
    }
    const mode = classifyPointerMode({ button: ev.button, altKey: ev.altKey });
    if (mode === "none") return;
    try {
      element.setPointerCapture(ev.pointerId);
    } catch {
      // pointer capture can fail in jsdom-like environments; ignore
    }
    pointer = {
      mode,
      pointerId: ev.pointerId,
      lastX: ev.clientX,
      lastY: ev.clientY,
    };
  };
  const onPointerMove = (ev: PointerEvent) => {
    if (!pointer || pointer.pointerId !== ev.pointerId) return;
    const dx = ev.clientX - pointer.lastX;
    const dy = ev.clientY - pointer.lastY;
    pointer.lastX = ev.clientX;
    pointer.lastY = ev.clientY;
    const next =
      pointer.mode === "orbit"
        ? orbitBy(cameraRef.current, dx * -0.01, dy * 0.01)
        : panBy(cameraRef.current, dx * -0.05, dy * 0.05);
    commit(next, null);
  };
  const onPointerUp = (ev: PointerEvent) => {
    if (!pointer || pointer.pointerId !== ev.pointerId) return;
    try {
      element.releasePointerCapture(ev.pointerId);
    } catch {
      // best-effort release
    }
    pointer = null;
  };
  const onWheel = (ev: WheelEvent) => {
    ev.preventDefault();
    const delta = ev.deltaY > 0 ? 0.1 : -0.1;
    commit(zoomBy(cameraRef.current, delta), null);
  };
  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
  };
  element.addEventListener("pointerdown", onPointerDown);
  element.addEventListener("pointermove", onPointerMove);
  element.addEventListener("pointerup", onPointerUp);
  element.addEventListener("pointercancel", onPointerUp);
  element.addEventListener("wheel", onWheel, { passive: false });
  element.addEventListener("contextmenu", onContextMenu);
  return () => {
    element.removeEventListener("pointerdown", onPointerDown);
    element.removeEventListener("pointermove", onPointerMove);
    element.removeEventListener("pointerup", onPointerUp);
    element.removeEventListener("pointercancel", onPointerUp);
    element.removeEventListener("wheel", onWheel);
    element.removeEventListener("contextmenu", onContextMenu);
  };
}

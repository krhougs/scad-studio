import { useCallback, useEffect, useRef, useState } from "react";
import type { CameraPreset, CameraState } from "../canvas/camera-state";
import {
  cadQuerySceneFromHandle,
  type CadQueryMeshHandleLike,
  type CadQueryScenePayload,
} from "./cadquery-mesh";
import {
  cadQuerySelectionKey,
  selectionRefFromCadQueryPick,
  updateCadQuerySelection,
  type CadQueryPickTarget,
  type CadQuerySelectionMode,
} from "./cadquery-selection";
import type { MeshInfo } from "./mesh-info";
import { createMeshViewer, type MeshViewerHandle } from "./mesh-three";
import type { MeshViewerOptions } from "./viewer-options";
import type { SelectionRef } from "@budn/app-server-protocol";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";

type CadQueryViewerProps = {
  resultId: string;
  client: CadQueryViewerClient;
  label: string;
  selectionMode: CadQuerySelectionMode;
  viewerOptions?: MeshViewerOptions;
  cameraPreset?: CameraPreset | null;
  cameraOverride?: CameraState | null;
  interactionMode?: "select" | "preview";
  selectionSnapshot?: SelectionUpdateRequest | null;
  refreshSignal?: number;
  onScene?: (scene: CadQueryScenePayload | null) => void;
  onPreviewStatus?: (status: string) => void;
  onInfo?: (info: MeshInfo | null) => void;
  onCameraChange?: (camera: CameraState) => void;
};

type CadQueryViewerClient = {
  dispatchCadQueryResultGet(params: unknown): Promise<unknown>;
  takeCadQueryMesh(resultId: string): CadQueryMeshHandleLike | null;
  dispatchSelectionUpdate(params: unknown): Promise<unknown>;
};

type PendingSelection = {
  ref: SelectionRef;
  additive: boolean;
};

type CadQueryLoadInput = {
  cameraPreset?: CameraPreset | null;
  client: CadQueryViewerClient;
  resultId: string;
  onInfo?: (info: MeshInfo | null) => void;
  onPick: (pick: CadQueryPickTarget) => void;
  onPreviewStatus?: (status: string) => void;
  onScene?: (scene: CadQueryScenePayload | null) => void;
  sceneRef: React.MutableRefObject<CadQueryScenePayload | null>;
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>;
};

export function CadQueryViewer(props: CadQueryViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const viewerRef = useRef<MeshViewerHandle | null>(null);
  const sceneRef = useRef<CadQueryScenePayload | null>(null);
  const [mode, setMode] = useState<CadQuerySelectionMode>(props.selectionMode);
  const interactionMode = props.interactionMode ?? "select";
  const selection = useCadQuerySelection(
    props.client,
    sceneRef,
    props.selectionSnapshot,
  );

  useViewerLifecycle(canvasRef, viewerRef, props.onCameraChange);
  useViewerOptions(viewerRef, props.viewerOptions);
  useSelectionEnabled(viewerRef, interactionMode === "select");
  useSelectionMode(viewerRef, mode);
  useSelectionKeys(
    viewerRef,
    interactionMode === "select" ? selection.selectedKeys : [],
  );
  useCameraEffects(props, viewerRef);
  useCadQueryResultLoad(props, viewerRef, sceneRef, selection.handlePick);

  return (
    <CadQueryViewerFrame
      canvasRef={canvasRef}
      label={props.label}
      mode={mode}
      onMode={setMode}
      pending={selection.pending}
      activeSelection={selection.activeSelection}
      interactionMode={interactionMode}
      onCancelPending={selection.cancelPending}
      onConfirmPending={selection.confirmPending}
    />
  );
}

function useCadQuerySelection(
  client: CadQueryViewerClient,
  sceneRef: React.MutableRefObject<CadQueryScenePayload | null>,
  selectionSnapshot: SelectionUpdateRequest | null | undefined,
) {
  const selectionsRef = useRef<SelectionRef[]>([]);
  const [pending, setPending] = useState<PendingSelection | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<string[]>([]);
  const [activeSelection, setActiveSelection] =
    useState<SelectionRef | null>(null);
  const dispatchSelection = useCallback(
    (ref: SelectionRef, additive: boolean) => {
      const next = updateCadQuerySelection(
        selectionsRef.current,
        ref,
        additive,
      );
      selectionsRef.current = next;
      setActiveSelection(ref);
      setSelectedKeys(next.map(cadQuerySelectionKey));
      void client.dispatchSelectionUpdate({
        selections: next,
        active_index: next.length - 1,
      });
    },
    [client],
  );
  useEffect(() => {
    const selections = selectionSnapshot?.selections ?? [];
    selectionsRef.current = selections;
    setSelectedKeys(selections.map(cadQuerySelectionKey));
    setActiveSelection(activeSelectionFromSnapshot(selectionSnapshot));
  }, [selectionSnapshot]);
  const handlePick = useCallback(
    (pick: CadQueryPickTarget) => {
      const scene = sceneRef.current;
      if (!scene) return;
      const ref = selectionRefFromCadQueryPick(scene, pick);
      if (ref.ambiguous) setPending({ ref, additive: pick.additive });
      else dispatchSelection(ref, pick.additive);
    },
    [dispatchSelection, sceneRef],
  );
  return {
    selectedKeys,
    activeSelection,
    pending,
    handlePick,
    cancelPending: () => setPending(null),
    confirmPending: () => {
      if (pending) dispatchSelection(pending.ref, pending.additive);
      setPending(null);
    },
  };
}

function activeSelectionFromSnapshot(
  snapshot: SelectionUpdateRequest | null | undefined,
): SelectionRef | null {
  const selections = snapshot?.selections ?? [];
  if (selections.length === 0) return null;
  const active = snapshot?.active_index;
  if (typeof active === "number" && active >= 0 && active < selections.length) {
    return selections[active] ?? null;
  }
  return selections[selections.length - 1] ?? null;
}

function CadQueryViewerFrame(props: {
  canvasRef: React.MutableRefObject<HTMLCanvasElement | null>;
  label: string;
  mode: CadQuerySelectionMode;
  pending: PendingSelection | null;
  activeSelection: SelectionRef | null;
  interactionMode: "select" | "preview";
  onMode: (mode: CadQuerySelectionMode) => void;
  onCancelPending: () => void;
  onConfirmPending: () => void;
}) {
  return (
    <div
      className="viewer viewer--mesh viewer--cadquery"
      data-testid="cadquery-viewer"
    >
      {props.interactionMode === "select" ? (
        <>
          <CadQuerySelectionDock mode={props.mode} onMode={props.onMode} />
          <CadQuerySelectionStatus selection={props.activeSelection} />
        </>
      ) : null}
      <canvas
        ref={props.canvasRef}
        className="mesh-viewer__canvas"
        data-testid="cadquery-canvas"
        data-label={props.label}
      />
      {props.pending && props.interactionMode === "select" ? (
        <AmbiguousSelectionDialog
          selection={props.pending.ref}
          onCancel={props.onCancelPending}
          onConfirm={props.onConfirmPending}
        />
      ) : null}
    </div>
  );
}

function CadQuerySelectionStatus(props: { selection: SelectionRef | null }) {
  if (!props.selection) return null;
  return (
    <div
      className="cadquery-selection-status"
      data-testid="cadquery-selection-status"
    >
      <span>{props.selection.kind}</span>
      <code>{selectionStatusText(props.selection)}</code>
    </div>
  );
}

function selectionStatusText(selection: SelectionRef): string {
  return selection.candidate_feature_ref ?? selection.ref_text;
}

function useViewerLifecycle(
  canvasRef: React.MutableRefObject<HTMLCanvasElement | null>,
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  onCameraChange: ((camera: CameraState) => void) | undefined,
) {
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const viewer = createMeshViewer(canvas);
    viewerRef.current = viewer;
    if (onCameraChange) viewer.onCameraChange(onCameraChange);
    const parent = canvas.parentElement;
    const resize = () => resizeViewer(viewer, parent);
    resize();
    const observer = createResizeObserver(resize, parent);
    return () => {
      observer?.disconnect();
      viewer.dispose();
      viewerRef.current = null;
    };
  }, [canvasRef, onCameraChange, viewerRef]);
}

function useCadQueryResultLoad(
  props: CadQueryViewerProps,
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  sceneRef: React.MutableRefObject<CadQueryScenePayload | null>,
  onPick: (pick: CadQueryPickTarget) => void,
) {
  const {
    cameraPreset,
    client,
      resultId,
    refreshSignal,
    onInfo,
      onPreviewStatus,
      onScene,
  } = props;
  useEffect(() => {
    let cancelled = false;
    loadCadQueryResult(
      {
        cameraPreset,
        client,
        resultId,
        onInfo,
        onPick,
      onPreviewStatus,
        onScene,
        sceneRef,
        viewerRef,
      },
      () => cancelled,
    );
    return () => {
      cancelled = true;
    };
  }, [
    cameraPreset,
    client,
    onInfo,
    onPick,
    onPreviewStatus,
    onScene,
    refreshSignal,
    resultId,
    sceneRef,
    viewerRef,
  ]);
}

function loadCadQueryResult(
  input: CadQueryLoadInput,
  isCancelled: () => boolean,
): void {
  input.onPreviewStatus?.("cadquery pending");
  input.client
    .dispatchCadQueryResultGet({ result_id: input.resultId })
    .then(() => {
      if (isCancelled()) return;
      const handle = input.client.takeCadQueryMesh(input.resultId);
      if (!handle) throw new Error("cadquery mesh missing");
      const scene = cadQuerySceneFromHandle(handle);
      input.sceneRef.current = scene;
      input.onScene?.(scene);
      const info =
        input.viewerRef.current?.setCadQueryMesh(scene, {
          frame: true,
          preset: input.cameraPreset ?? "iso",
          onPick: input.onPick,
        }) ?? null;
      input.onPreviewStatus?.("cadquery ready");
      input.onInfo?.(info);
    })
    .catch((err) => {
      if (isCancelled()) return;
      input.onPreviewStatus?.(`cadquery error: ${errorMessage(err)}`);
      input.onInfo?.(null);
      input.onScene?.(null);
    });
}

function useViewerOptions(
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  viewerOptions: MeshViewerOptions | undefined,
) {
  useEffect(() => {
    if (viewerOptions) viewerRef.current?.setOptions(viewerOptions);
  }, [viewerOptions, viewerRef]);
}

function useSelectionMode(
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  selectionMode: CadQuerySelectionMode,
) {
  useEffect(() => {
    viewerRef.current?.setCadQuerySelectionMode(selectionMode);
  }, [selectionMode, viewerRef]);
}

function useSelectionEnabled(
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  enabled: boolean,
) {
  useEffect(() => {
    viewerRef.current?.setCadQuerySelectionEnabled(enabled);
  }, [enabled, viewerRef]);
}

function useSelectionKeys(
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
  selectedKeys: string[],
) {
  useEffect(() => {
    viewerRef.current?.setCadQuerySelectedKeys(selectedKeys);
  }, [selectedKeys, viewerRef]);
}

function useCameraEffects(
  props: CadQueryViewerProps,
  viewerRef: React.MutableRefObject<MeshViewerHandle | null>,
) {
  useEffect(() => {
    if (props.cameraOverride)
      viewerRef.current?.setCamera(props.cameraOverride);
  }, [props.cameraOverride, viewerRef]);
}

function CadQuerySelectionDock(props: {
  mode: CadQuerySelectionMode;
  onMode: (mode: CadQuerySelectionMode) => void;
}) {
  return (
    <div className="cadquery-select-dock" data-testid="cadquery-select-dock">
      {(["part", "face", "edge", "vertex", "assembly"] as const).map((mode) => (
        <button
          key={mode}
          type="button"
          className={props.mode === mode ? "active" : undefined}
          onClick={() => props.onMode(mode)}
        >
          {mode}
        </button>
      ))}
    </div>
  );
}

function AmbiguousSelectionDialog(props: {
  selection: SelectionRef;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const target =
    props.selection.candidate_feature_ref ?? props.selection.ref_text;
  return (
    <div className="cadquery-ambiguous" role="dialog">
      <span>ambiguous feature</span>
      <code>{target}</code>
      <button type="button" onClick={props.onCancel}>
        cancel
      </button>
      <button type="button" onClick={props.onConfirm}>
        confirm
      </button>
    </div>
  );
}

function resizeViewer(
  viewer: MeshViewerHandle,
  parent: HTMLElement | null,
): void {
  const rect = parent?.getBoundingClientRect();
  if (rect) viewer.resize(rect.width, rect.height, window.devicePixelRatio);
}

function createResizeObserver(
  resize: () => void,
  parent: HTMLElement | null,
): ResizeObserver | null {
  if (!parent || typeof ResizeObserver === "undefined") return null;
  const observer = new ResizeObserver(resize);
  observer.observe(parent);
  return observer;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

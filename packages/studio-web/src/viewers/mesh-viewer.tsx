// Mesh viewer: sends a PreviewRequest for the path and renders the resulting
// summary plus a canvas slot. Reuses the Phase 5 pipeline — mesh bytes are
// handed to wasm `mesh_decode`, and the existing CanvasZone-level canvas is
// currently the only renderer surface.

import { useEffect, useState } from "react";
import * as WasmMod from "@scad-studio/studio-web-wasm";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "./file-read-decoder";

type MeshViewerProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  onPreviewStatus?: (status: string) => void;
};

type MeshState =
  | { kind: "pending" }
  | { kind: "ready"; vertices: number; indices: number }
  | { kind: "error"; message: string };

export function MeshViewer({
  path,
  client,
  label,
  onPreviewStatus,
}: MeshViewerProps) {
  const [state, setState] = useState<MeshState>({ kind: "pending" });

  useEffect(() => {
    let cancelled = false;
    setState({ kind: "pending" });
    onPreviewStatus?.("preview pending");
    client
      .dispatchPreviewRequest({
        source: path,
        defines: [],
        kind: "geometry_artifact",
        configured_openscad_path: null,
      })
      .then((payload) => {
        if (cancelled) return;
        const summary = extractMeshCounts(payload);
        setState(
          summary
            ? { kind: "ready", ...summary }
            : { kind: "ready", vertices: 0, indices: 0 },
        );
        onPreviewStatus?.("preview ready");
      })
      .catch((err) => {
        if (cancelled) return;
        const msg = describeFileReadError(err);
        setState({ kind: "error", message: msg });
        onPreviewStatus?.(`preview error: ${msg}`);
      });
    return () => {
      cancelled = true;
    };
  }, [client, path, onPreviewStatus]);

  return (
    <div
      className="viewer viewer--mesh"
      data-testid="mesh-viewer"
      data-label={label}
    >
      <p className="viewer__status" data-testid="mesh-status">
        {state.kind === "pending"
          ? "preview pending"
          : state.kind === "ready"
            ? `preview ready | vertices: ${state.vertices} | indices: ${state.indices}`
            : `preview error: ${state.message}`}
      </p>
    </div>
  );
}

function extractMeshCounts(
  payload: unknown,
): { vertices: number; indices: number } | null {
  if (!payload || typeof payload !== "object") return null;
  const outer = payload as Record<string, unknown>;
  const ready =
    (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const artifact = ready["artifact"] as Record<string, unknown> | undefined;
  if (!artifact) return null;
  const format = artifact["format"];
  const inner = artifact["payload"] as Record<string, unknown> | undefined;
  if (!inner) return null;
  if (format === "mesh") {
    const positions = inner["positions"];
    const indices = inner["indices"];
    if (Array.isArray(positions) && Array.isArray(indices)) {
      return { vertices: positions.length, indices: indices.length };
    }
  }
  if (format === "three_mf") {
    return summarizeThreeMf(inner);
  }
  return null;
}

function summarizeThreeMf(
  inner: Record<string, unknown>,
): { vertices: number; indices: number } | null {
  const bytes = inner["bytes"];
  const u8 =
    bytes instanceof Uint8Array
      ? bytes
      : Array.isArray(bytes)
        ? Uint8Array.from(bytes as number[])
        : null;
  if (!u8) return null;
  try {
    const handle = WasmMod.mesh_decode(u8);
    WasmMod.mesh_destroy(handle);
    return { vertices: Math.floor(u8.length / 32), indices: u8.length };
  } catch (err) {
    console.warn("mesh_decode failed:", err);
    return null;
  }
}

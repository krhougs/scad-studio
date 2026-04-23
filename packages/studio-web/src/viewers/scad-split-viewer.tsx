// Scad split viewer: source (FileRead) on the left, preview status on the
// right. Web server currently denies `.scad` reads (source secrecy, see
// docs/web-platform-limits.md); that error is surfaced as the source pane
// content rather than a crash, so the smoke case can still assert both panes.

import { useEffect, useState } from "react";
import * as WasmMod from "@scad-studio/studio-web-wasm";
import { WasmClient } from "../wasm-bridge";
import {
  decodeFileRead,
  describeFileReadError,
} from "./file-read-decoder";

type ScadSplitViewerProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  defines?: string[];
  onPreviewStatus?: (status: string) => void;
};

type ScadPreviewStatus =
  | { kind: "pending" }
  | { kind: "ready"; vertices: number; indices: number }
  | { kind: "error"; message: string };

export function ScadSplitViewer({
  path,
  client,
  label,
  defines,
  onPreviewStatus,
}: ScadSplitViewerProps) {
  const [source, setSource] = useState<string | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [preview, setPreview] = useState<ScadPreviewStatus>({ kind: "pending" });

  useEffect(() => {
    let cancelled = false;
    setSource(null);
    setSourceError(null);
    setPreview({ kind: "pending" });
    onPreviewStatus?.("preview pending");

    client
      .dispatchFileRead({ path })
      .then((response) => {
        if (cancelled) return;
        const decoded = decodeFileRead(response);
        if (!decoded) {
          setSourceError("unexpected FileRead payload");
          return;
        }
        if (decoded.kind !== "utf8") {
          setSourceError("scad file is not utf-8 text");
          return;
        }
        setSource(decoded.text);
      })
      .catch((err) => {
        if (cancelled) return;
        setSourceError(
          `source unavailable: ${describeFileReadError(err)}`,
        );
      });

    client
      .dispatchPreviewRequest({
        source: path,
        defines: defines ?? [],
        kind: "geometry_artifact",
        configured_openscad_path: null,
      })
      .then((payload) => {
        if (cancelled) return;
        const summary = extractMeshCounts(payload);
        if (summary) {
          setPreview({ kind: "ready", ...summary });
        } else {
          setPreview({ kind: "ready", vertices: 0, indices: 0 });
        }
        onPreviewStatus?.("preview ready");
      })
      .catch((err) => {
        if (cancelled) return;
        const message = describeFileReadError(err);
        setPreview({ kind: "error", message });
        onPreviewStatus?.(`preview error: ${message}`);
      });

    return () => {
      cancelled = true;
    };
  }, [client, path, defines, onPreviewStatus]);

  const sourceBody =
    source != null
      ? source
      : sourceError != null
        ? sourceError
        : "reading…";
  const previewText =
    preview.kind === "pending"
      ? "preview pending"
      : preview.kind === "ready"
        ? `preview ready | vertices: ${preview.vertices} | indices: ${preview.indices}`
        : `preview error: ${preview.message}`;

  return (
    <div
      className="viewer viewer--scad-split"
      data-testid="scad-split-viewer"
      data-label={label}
    >
      <div className="scad-split__pane">
        <h5 className="scad-split__title">source</h5>
        <pre className="scad-split__code" data-testid="scad-source">
          <code>{sourceBody}</code>
        </pre>
      </div>
      <div className="scad-split__pane">
        <h5 className="scad-split__title">preview</h5>
        <p
          className={`scad-split__status ${preview.kind}`}
          data-testid="scad-preview-status"
        >
          {previewText}
        </p>
      </div>
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

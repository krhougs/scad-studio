import { useEffect, useMemo, useState } from "react";
import type { AppConfigShape, SlicerRow } from "../config/app-config";
import { configuredSlicerRecords } from "../config/app-config";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "../viewers/file-read-decoder";
import { resolveSiblingOutputPath } from "./protocol-paths";

export type SlicerEntry = {
  name: string;
  path: string;
};

type SlicerPanelProps = {
  client: WasmClient | null;
  source?: unknown;
  defaultFilename?: string;
  defines?: string[];
  config: AppConfigShape | null;
  onStatus?: (status: string) => void;
};

type LoadState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; slicers: SlicerEntry[] }
  | { kind: "error"; message: string };

export function SlicerPanel({
  client,
  source,
  defaultFilename,
  defines,
  config,
  onStatus,
}: SlicerPanelProps) {
  const [state, setState] = useState<LoadState>({ kind: "idle" });
  const [actionStatus, setActionStatus] = useState<string>("idle");
  const configured = useMemo(
    () => configuredSlicerRecords(config ?? {}),
    [config],
  );

  useEffect(() => {
    if (!client) return;
    let cancelled = false;
    setState({ kind: "loading" });
    client
      .dispatchSlicerList({ configured })
      .then((response) => {
        if (cancelled) return;
        const slicers = extractSlicers(response);
        setState({ kind: "ready", slicers });
      })
      .catch((err) => {
        if (cancelled) return;
        setState({ kind: "error", message: describeFileReadError(err) });
      });
    return () => {
      cancelled = true;
    };
  }, [client, configured]);

  const sendToSlicer = async (slicer: SlicerRow) => {
    if (!client || !source) return;
    const effective = defaultFilename?.trim() || "export.stl";
    setActionStatus(`sending to ${slicer.name}`);
    onStatus?.(`sending to ${slicer.name}`);
    try {
      const outputPath = await resolveSiblingOutputPath(source, effective);
      await client.dispatchExportRun({
        configured_openscad_path: config?.openscad_path ?? null,
        configured_slicers: configured,
        source,
        defines: defines ?? [],
        output_path: outputPath,
        format: "stl",
        slicer_name: slicer.name,
      });
      const status = `sent to ${slicer.name}: ${effective}`;
      setActionStatus(status);
      onStatus?.(status);
    } catch (err) {
      const message = describeFileReadError(err);
      const status = `export error: ${message}`;
      setActionStatus(status);
      onStatus?.(status);
    }
  };

  return (
    <section
      className="panel panel--slicer"
      aria-label="slicers"
      data-testid="slicer-panel"
    >
      <header className="panel__head">
        <h5 className="panel__title">slicers</h5>
        <span className="panel__sub" data-testid="slicer-status">
          {actionStatus}
        </span>
      </header>
      {state.kind === "loading" ? (
        <p className="panel__empty" data-testid="slicer-loading">
          loading…
        </p>
      ) : null}
      {state.kind === "error" ? (
        <p className="panel__empty" data-testid="slicer-error">
          error: {state.message}
        </p>
      ) : null}
      {state.kind === "ready" ? (
        <ul className="panel__list" data-testid="slicer-list">
          {state.slicers.length === 0 ? (
            <li className="panel__empty" data-testid="slicer-empty">
              no slicer configured
            </li>
          ) : (
            state.slicers.map((entry) => (
              <li
                key={`${entry.name}:${entry.path}`}
                className="panel__row"
                data-testid={`slicer-row-${entry.name}`}
              >
                <span className="panel__label">{entry.name}</span>
                <span
                  className="panel__meta"
                  data-testid={`slicer-path-${entry.name}`}
                >
                  {entry.path || "—"}
                </span>
                <button
                  type="button"
                  className="btn btn--ghost btn--sm"
                  onClick={() => sendToSlicer(entry)}
                  disabled={!client || !source}
                  data-testid={`slicer-send-${entry.name}`}
                >
                  send to slicer
                </button>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </section>
  );
}

function extractSlicers(response: unknown): SlicerEntry[] {
  if (!response || typeof response !== "object") return [];
  const outer = response as Record<string, unknown>;
  const inner =
    (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const slicers = inner["slicers"];
  if (!Array.isArray(slicers)) return [];
  const out: SlicerEntry[] = [];
  for (const item of slicers) {
    if (!item || typeof item !== "object") continue;
    const row = item as Record<string, unknown>;
    const name = row["name"];
    const path = row["path"];
    if (typeof name === "string") {
      out.push({
        name,
        path: typeof path === "string" ? path : String(path ?? ""),
      });
    }
  }
  return out;
}

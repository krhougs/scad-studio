// Slicer panel: dispatches SlicerList once on mount and renders the server's
// reply. We never attempt to launch a slicer from the browser; the server
// owns that (see docs/web-platform-limits.md §1). Empty list is a first-class
// UI state — "no slicer configured" beats a silent empty panel.

import { useEffect, useState } from "react";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "../viewers/file-read-decoder";

export type SlicerEntry = {
  name: string;
  path: string;
};

type SlicerPanelProps = {
  client: WasmClient | null;
};

type LoadState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; slicers: SlicerEntry[] }
  | { kind: "error"; message: string };

export function SlicerPanel({ client }: SlicerPanelProps) {
  const [state, setState] = useState<LoadState>({ kind: "idle" });

  useEffect(() => {
    if (!client) return;
    let cancelled = false;
    setState({ kind: "loading" });
    client
      .dispatchSlicerList({ configured: [] })
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
  }, [client]);

  return (
    <section
      className="panel panel--slicer"
      aria-label="slicers"
      data-testid="slicer-panel"
    >
      <header className="panel__head">
        <h5 className="panel__title">slicers</h5>
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

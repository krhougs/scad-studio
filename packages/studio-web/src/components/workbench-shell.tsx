// Minimal workbench shell. Phase 3 only proves the end-to-end flow:
// open WebSocket → handshake → workspace.current → workspace.list → preview.request.
// Phase 4 will replace this with the five-zone Buddin layout.

import { useEffect, useMemo, useRef, useState } from "react";
import { BrowserWebSocketTransport } from "../transport/websocket-transport";
import { WasmClient } from "../wasm-bridge";
import { useCanvasRendererController } from "../canvas/renderer-controller";

type Phase =
  | "idle"
  | "connecting"
  | "handshaking"
  | "ready"
  | "preview-pending"
  | "preview-ready"
  | "preview-error"
  | "error";

type Snapshot = {
  workspace_current?: {
    workspace_id?: unknown;
    root_name?: string;
  } | null;
  workspace_list?: {
    directory?: unknown;
    entries?: Array<{ path: unknown; kind: "directory" | "file" }>;
  } | null;
  transport_status?: string;
} | null;

function resolveWsUrl(): string {
  const search = new URLSearchParams(window.location.search);
  const fromQuery = search.get("ws");
  if (fromQuery) return fromQuery;
  const fromEnv = import.meta.env.VITE_WS_URL;
  if (typeof fromEnv === "string" && fromEnv.length > 0) return fromEnv;
  return "ws://127.0.0.1:38421";
}

export function WorkbenchShell() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderer = useCanvasRendererController(canvasRef);

  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string>("");
  const [snapshot, setSnapshot] = useState<Snapshot>(null);

  const wsUrl = useMemo(() => resolveWsUrl(), []);
  const clientRef = useRef<WasmClient | null>(null);
  const transportRef = useRef<BrowserWebSocketTransport | null>(null);

  useEffect(() => {
    let disposed = false;
    setPhase("connecting");

    const client = new WasmClient({
      onSnapshotDirty: () => {
        if (disposed) return;
        const snap = client.snapshot() as Snapshot;
        setSnapshot(snap);
      },
      onHandshakeAccepted: () => {
        if (disposed) return;
        void runInitialFlow(client)
          .then((result) => {
            if (disposed) return;
            if (result.kind === "ready") {
              setPhase("ready");
              setMessage(result.message);
            } else {
              setPhase("preview-error");
              setMessage(result.message);
            }
          })
          .catch((err) => {
            if (disposed) return;
            setPhase("error");
            setMessage(describeError(err));
          });
      },
    });
    clientRef.current = client;

    const transport = new BrowserWebSocketTransport(wsUrl, {
      onOpen: () => {
        if (disposed) return;
        setPhase("handshaking");
        client.setSender((bytes) => transport.send(bytes));
        try {
          client.beginHandshake({
            capabilities: {
              client_name: "studio-web",
              platform: "web",
              protocol_version: { min: 1, max: 1 },
              file_read: { denied_extensions: [".scad", ".stl", ".3mf"] },
              supported_preview_kinds: ["geometry_artifact"],
            },
          });
          client.pump();
        } catch (err) {
          setPhase("error");
          setMessage(describeError(err));
        }
      },
      onMessage: (bytes) => {
        if (disposed) return;
        try {
          client.receiveInbound(bytes);
        } catch (err) {
          setMessage(describeError(err));
        }
      },
      onClose: (reason) => {
        if (disposed) return;
        client.setSender(null);
        client.markTransportClosed(reason);
        client.pump();
      },
      onError: (msg) => {
        if (disposed) return;
        setMessage(msg);
      },
    });
    transportRef.current = transport;
    transport.start();

    const tickHandle = window.setInterval(() => {
      if (disposed) return;
      client.pump();
    }, 200);

    return () => {
      disposed = true;
      window.clearInterval(tickHandle);
      transport.stop();
      client.destroy();
      clientRef.current = null;
      transportRef.current = null;
    };
  }, [wsUrl]);

  const handlePreview = (path: unknown) => {
    const client = clientRef.current;
    if (!client) return;
    setPhase("preview-pending");
    setMessage("preview pending");
    client
      .dispatchPreviewRequest({
        source: path,
        defines: [],
        kind: "geometry_artifact",
        configured_openscad_path: null,
      })
      .then(() => {
        setPhase("preview-ready");
        setMessage("preview ready");
      })
      .catch((err) => {
        setPhase("preview-error");
        setMessage(`preview error: ${describeError(err)}`);
      });
  };

  const entries = snapshot?.workspace_list?.entries ?? [];
  const rootName = snapshot?.workspace_current?.root_name ?? "(loading)";

  return (
    <section style={{ padding: "16px", fontFamily: "ui-monospace, monospace" }}>
      <header>
        <h1 style={{ margin: 0 }}>scad-studio web</h1>
        <p data-testid="phase">phase: {phase}</p>
        <p data-testid="message">message: {message}</p>
        <p data-testid="ws-url">ws: {wsUrl}</p>
      </header>
      <section>
        <h2>workspace</h2>
        <p data-testid="workspace-name">{rootName}</p>
      </section>
      <section>
        <h2>entries</h2>
        <ul data-testid="entries">
          {entries.map((entry, idx) => {
            const label = formatPath(entry.path);
            return (
              <li key={`${label}-${idx}`}>
                <button
                  type="button"
                  data-testid={`entry-${label}`}
                  onClick={() => handlePreview(entry.path)}
                >
                  {entry.kind}: {label}
                </button>
              </li>
            );
          })}
        </ul>
      </section>
      <section>
        <h2>renderer</h2>
        <p data-testid="renderer-status">
          {renderer.ready
            ? "ready"
            : `stub (${renderer.lastError ?? "not created yet"})`}
        </p>
        <canvas
          ref={canvasRef}
          width={640}
          height={360}
          style={{ border: "1px solid #444" }}
        />
      </section>
    </section>
  );
}

type InitialResult =
  | { kind: "ready"; message: string }
  | { kind: "error"; message: string };

async function runInitialFlow(client: WasmClient): Promise<InitialResult> {
  try {
    await client.dispatchWorkspaceCurrent();
  } catch (err) {
    return { kind: "error", message: `workspace.current: ${describeError(err)}` };
  }
  try {
    await client.dispatchWorkspaceList({ directory: null });
  } catch (err) {
    return { kind: "error", message: `workspace.list: ${describeError(err)}` };
  }
  return { kind: "ready", message: "workspace ready" };
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "object" && err !== null) {
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return String(err);
}

function formatPath(path: unknown): string {
  if (typeof path === "string") return path;
  if (path && typeof path === "object") {
    const handle = path as Record<string, unknown>;
    for (const key of ["value", "path", "id"]) {
      const v = handle[key];
      if (typeof v === "string") return v;
    }
    try {
      return JSON.stringify(path);
    } catch {
      return "[path]";
    }
  }
  return String(path ?? "");
}

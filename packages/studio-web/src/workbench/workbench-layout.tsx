// Workbench layout: CSS Grid 五区外框 + transport/protocol 生命周期接线。
// Phase 5 补齐：
//   - handshake ack 后自动订阅 directory watch
//   - watch event 到达 → 重拉 root workspace_list + 已展开目录列表
//   - Inspector 树形递归：directory entry 点击展开/折叠
//   - preview_request 的 PreviewReadyResponse 如果有 mesh / 3mf bytes，
//     调用 wasm mesh_decode 验证 bridge 接收能力并展示元数据
//
// 本文件只做 React 壳层的 wire-up 与 UI 派发；所有协议状态机在 wasm 里。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as WasmMod from "@scad-studio/studio-web-wasm";
import { WasmClient } from "../wasm-bridge";
import { CanvasZone } from "./canvas-zone";
import { ChatZone } from "./chat-zone";
import {
  Inspector,
  type InspectorDirectoryNode,
  type InspectorEntry,
  type InspectorMeshSummary,
} from "./inspector";
import { pathKey, pathLabel } from "./path-utils";
import { Rail } from "./rail";
import { Topbar, type TopbarStatus } from "./topbar";
import {
  createTransport,
  describeError,
  buildClientCallbacks,
} from "./workbench-wiring";

type Phase =
  | "idle"
  | "connecting"
  | "handshaking"
  | "ready"
  | "preview-pending"
  | "preview-ready"
  | "preview-error"
  | "error";

type ProtocolEntry = { path: unknown; kind: "directory" | "file" };

type Snapshot = {
  workspace_current?: {
    workspace_id?: unknown;
    root_name?: string;
  } | null;
  workspace_list?: {
    directory?: unknown;
    entries?: ProtocolEntry[];
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

function phaseToStatus(phase: Phase): TopbarStatus {
  switch (phase) {
    case "idle":
      return "idle";
    case "connecting":
    case "handshaking":
      return "connecting";
    case "ready":
    case "preview-ready":
      return "ready";
    case "preview-pending":
      return "busy";
    case "error":
    case "preview-error":
      return "error";
  }
}

function toInspectorEntry(entry: ProtocolEntry): InspectorEntry {
  return {
    label: pathLabel(entry.path) || "(unnamed)",
    path: entry.path,
    kind: entry.kind,
  };
}

function extractMeshSummary(
  payload: unknown,
  targetLabel: string,
  wasm: typeof WasmMod,
): InspectorMeshSummary | null {
  if (!payload || typeof payload !== "object") return null;
  const ready = payload as Record<string, unknown>;
  const artifact = ready["artifact"] as Record<string, unknown> | undefined;
  if (!artifact) return null;
  const format = artifact["format"];
  const inner = artifact["payload"] as Record<string, unknown> | undefined;
  if (!inner) return null;
  if (format === "mesh") {
    const positions = inner["positions"];
    const indices = inner["indices"];
    if (Array.isArray(positions) && Array.isArray(indices)) {
      return { label: targetLabel, vertices: positions.length, indices: indices.length };
    }
    return null;
  }
  if (format === "three_mf") {
    return summarizeThreeMfArtifact(inner, targetLabel, wasm);
  }
  return null;
}

function summarizeThreeMfArtifact(
  inner: Record<string, unknown>,
  targetLabel: string,
  wasm: typeof WasmMod,
): InspectorMeshSummary | null {
  const bytes = inner["bytes"];
  const u8 =
    bytes instanceof Uint8Array
      ? bytes
      : Array.isArray(bytes)
        ? Uint8Array.from(bytes as number[])
        : null;
  if (!u8) return null;
  try {
    const handle = wasm.mesh_decode(u8);
    wasm.mesh_destroy(handle);
    return {
      label: targetLabel,
      vertices: Math.floor(u8.length / 32),
      indices: u8.length,
    };
  } catch (err) {
    console.warn("mesh_decode failed:", err);
    return null;
  }
}

export function WorkbenchLayout() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string>("");
  const [snapshot, setSnapshot] = useState<Snapshot>(null);
  const [previewTarget, setPreviewTarget] = useState<string>("—");
  const [meshSummary, setMeshSummary] = useState<InspectorMeshSummary | null>(
    null,
  );
  const [expanded, setExpanded] = useState<Map<string, InspectorDirectoryNode>>(
    () => new Map(),
  );

  const wsUrl = useMemo(() => resolveWsUrl(), []);
  const clientRef = useRef<WasmClient | null>(null);
  const expandedRef = useRef<Map<string, InspectorDirectoryNode>>(new Map());
  const watchActiveRef = useRef(false);

  const setExpandedBoth = useCallback(
    (
      updater: (
        prev: Map<string, InspectorDirectoryNode>,
      ) => Map<string, InspectorDirectoryNode>,
    ) => {
      setExpanded((prev) => {
        const next = updater(prev);
        expandedRef.current = next;
        return next;
      });
    },
    [],
  );

  const refreshRootListing = useCallback((client: WasmClient) => {
    client.dispatchWorkspaceList({ directory: null }).catch((err) => {
      console.warn("workspace.list root refresh failed:", describeError(err));
    });
  }, []);

  const refreshExpandedDirectories = useCallback(
    (client: WasmClient) => {
      const snapshot = expandedRef.current;
      for (const [key, state] of snapshot) {
        if (state.loading) continue;
        client
          .dispatchWorkspaceList({ directory: state.path })
          .then((response) => {
            const entries = ((response as { entries?: ProtocolEntry[] })?.entries ?? [])
              .map(toInspectorEntry);
            setExpandedBoth((prev) => {
              const next = new Map(prev);
              next.set(key, { ...state, entries, loading: false, error: null });
              return next;
            });
          })
          .catch((err) => {
            setExpandedBoth((prev) => {
              const next = new Map(prev);
              next.set(key, { ...state, loading: false, error: describeError(err) });
              return next;
            });
          });
      }
    },
    [setExpandedBoth],
  );

  const handleExpandDirectory = useCallback(
    (entry: InspectorEntry) => {
      const client = clientRef.current;
      if (!client) return;
      const key = pathKey(entry.path);
      const pending: InspectorDirectoryNode = {
        key,
        label: entry.label,
        path: entry.path,
        entries: null,
        loading: true,
        error: null,
      };
      setExpandedBoth((prev) => {
        const next = new Map(prev);
        next.set(key, pending);
        return next;
      });
      client
        .dispatchWorkspaceList({ directory: entry.path })
        .then((response) => {
          const children = ((response as { entries?: ProtocolEntry[] })?.entries ?? [])
            .map(toInspectorEntry);
          setExpandedBoth((prev) => {
            const next = new Map(prev);
            next.set(key, { ...pending, entries: children, loading: false });
            return next;
          });
        })
        .catch((err) => {
          setExpandedBoth((prev) => {
            const next = new Map(prev);
            next.set(key, {
              ...pending,
              loading: false,
              error: describeError(err),
            });
            return next;
          });
        });
    },
    [setExpandedBoth],
  );

  const handleCollapseDirectory = useCallback(
    (entry: InspectorEntry) => {
      const key = pathKey(entry.path);
      setExpandedBoth((prev) => {
        if (!prev.has(key)) return prev;
        const next = new Map(prev);
        next.delete(key);
        return next;
      });
    },
    [setExpandedBoth],
  );

  useEffect(() => {
    let disposed = false;
    setPhase("connecting");
    const client = new WasmClient(
      buildClientCallbacks({
        onSnapshotDirty: () => {
          if (disposed) return;
          setSnapshot(client.snapshot() as Snapshot);
        },
        onHandshakeAccepted: () => {
          if (disposed) return;
          void onHandshakeAck(client, {
            setPhase,
            setMessage,
            watchActiveRef,
            disposedRef: () => disposed,
          });
        },
        onWatchEvent: () => {
          if (disposed) return;
          refreshRootListing(client);
          refreshExpandedDirectories(client);
        },
      }),
    );
    clientRef.current = client;

    const transport = createTransport({
      wsUrl,
      client,
      onHandshaking: () => {
        if (!disposed) setPhase("handshaking");
      },
      onTransportError: (msg) => {
        if (disposed) return;
        setPhase("error");
        setMessage(msg);
      },
      onTransportLost: (msg) => {
        if (disposed) return;
        setPhase("error");
        setMessage(msg);
      },
      onTransportReconnecting: (msg) => {
        if (disposed) return;
        setPhase("connecting");
        setMessage(msg);
      },
      onWatchReset: () => {
        watchActiveRef.current = false;
      },
    });
    transport.start();

    const tickHandle = window.setInterval(() => {
      if (!disposed) client.pump();
    }, 200);

    return () => {
      disposed = true;
      window.clearInterval(tickHandle);
      transport.stop();
      client.destroy();
      clientRef.current = null;
      watchActiveRef.current = false;
    };
  }, [wsUrl, refreshRootListing, refreshExpandedDirectories]);

  const entriesLoaded = snapshot?.workspace_list !== undefined;
  const entries: InspectorEntry[] = (snapshot?.workspace_list?.entries ?? []).map(
    toInspectorEntry,
  );
  const rootName = snapshot?.workspace_current?.root_name ?? "(loading)";

  const handlePreview = useCallback((entry: InspectorEntry) => {
    const client = clientRef.current;
    if (!client) return;
    setPreviewTarget(entry.label);
    setMeshSummary(null);
    setPhase("preview-pending");
    setMessage("preview pending");
    client
      .dispatchPreviewRequest({
        source: entry.path,
        defines: [],
        kind: "geometry_artifact",
        configured_openscad_path: null,
      })
      .then((payload) => {
        setPhase("preview-ready");
        setMessage("preview ready");
        const summary = extractMeshSummary(payload, entry.label, WasmMod);
        if (summary) setMeshSummary(summary);
      })
      .catch((err) => {
        setPhase("preview-error");
        setMessage(`preview error: ${describeError(err)}`);
      });
  }, []);

  return (
    <div className="workbench" data-testid="workbench-layout">
      <Topbar
        workspaceName={rootName}
        wsUrl={wsUrl}
        status={phaseToStatus(phase)}
        message={message}
      />
      <Rail />
      <ChatZone />
      <CanvasZone
        phase={phase}
        message={message}
        previewTargetLabel={previewTarget}
      />
      <Inspector
        rootName={rootName}
        entries={entries}
        entriesLoaded={entriesLoaded}
        onRequestPreview={handlePreview}
        onExpandDirectory={handleExpandDirectory}
        onCollapseDirectory={handleCollapseDirectory}
        previewTargetLabel={previewTarget}
        meshSummary={meshSummary}
        expandedDirectories={expanded}
        directoryKey={pathKey}
      />
    </div>
  );
}

type HandshakeCtx = {
  setPhase: (value: Phase) => void;
  setMessage: (value: string) => void;
  watchActiveRef: React.MutableRefObject<boolean>;
  disposedRef: () => boolean;
};

async function onHandshakeAck(
  client: WasmClient,
  ctx: HandshakeCtx,
): Promise<void> {
  try {
    await client.dispatchWorkspaceCurrent();
    await client.dispatchWorkspaceList({ directory: null });
  } catch (err) {
    if (ctx.disposedRef()) return;
    ctx.setPhase("preview-error");
    ctx.setMessage(`initial flow: ${describeError(err)}`);
    return;
  }
  if (ctx.disposedRef()) return;
  ctx.setPhase("ready");
  ctx.setMessage("workspace ready");
  if (!ctx.watchActiveRef.current) {
    try {
      client.subscribeDirectoryWatch({
        request: { directory: null },
        throttle_ms: 150,
      });
      ctx.watchActiveRef.current = true;
    } catch (err) {
      ctx.setMessage(`watch subscribe failed: ${describeError(err)}`);
    }
  }
}

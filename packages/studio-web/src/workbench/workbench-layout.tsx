// Workbench layout: CSS Grid 五区外框 + transport/protocol 生命周期接线。
// Phase 5 已接好 handshake / watch / inspector 树 / mesh_decode。
// Phase 6 补上文档标签系统（Tab Bar + DocumentTab Zustand 状态 + 多 viewer
// 挂载到 Canvas Zone）。Inspector 点击文件 → 按扩展名路由到对应 viewer tab；
// 不支持的扩展名仅更新状态条消息，不开 tab。
//
// 协议业务状态仍在 wasm 内；Zustand 只存 UI 壳状态（openTabs / activeTabId
// / sidePanelOpen 等）。viewer 自己发 FileRead / PreviewRequest，tab 只记
// id / label / path / kind。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WasmClient } from "../wasm-bridge";
import { useUiStore } from "../state/ui-store";
import { CanvasZone, type ViewPreset } from "./canvas-zone";
import { ChatZone } from "./chat-zone";
import {
  Inspector,
  type InspectorDirectoryNode,
  type InspectorEntry,
} from "./inspector";
import { LogPanel } from "./log-panel";
import { useLogBuffer } from "./use-log-buffer";
import { pathKey, pathLabel } from "./path-utils";
import { Rail } from "./rail";
import { resolveTabKind, extensionOf } from "./tab-kind";
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
      return "ready";
    case "error":
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

function extractWorkspaceListEntries(response: unknown): ProtocolEntry[] {
  if (!response || typeof response !== "object") return [];
  const outer = response as Record<string, unknown>;
  const inner = (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const entries = (inner["entries"] as ProtocolEntry[] | undefined) ?? [];
  return Array.isArray(entries) ? entries : [];
}

export function WorkbenchLayout() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string>("");
  const [snapshot, setSnapshot] = useState<Snapshot>(null);
  const [expanded, setExpanded] = useState<Map<string, InspectorDirectoryNode>>(
    () => new Map(),
  );
  const [rootEntries, setRootEntries] = useState<InspectorEntry[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [clientReady, setClientReady] = useState(false);
  const [activeView, setActiveView] = useState<ViewPreset>("iso");
  const [meshStats, setMeshStats] = useState<
    { vertices: number; indices: number } | null
  >(null);

  const openTabs = useUiStore((s) => s.openTabs);
  const activeTabId = useUiStore((s) => s.activeTabId);
  const openTab = useUiStore((s) => s.openTab);
  const closeTab = useUiStore((s) => s.closeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);

  const wsUrl = useMemo(() => resolveWsUrl(), []);
  const clientRef = useRef<WasmClient | null>(null);
  const expandedRef = useRef<Map<string, InspectorDirectoryNode>>(new Map());
  const watchActiveRef = useRef(false);
  const log = useLogBuffer();
  const logRef = useRef(log);
  logRef.current = log;
  const [scadRefreshSignal, setScadRefreshSignal] = useState(0);
  const openTabsRef = useRef(openTabs);
  openTabsRef.current = openTabs;
  const activeTabIdRef = useRef(activeTabId);
  activeTabIdRef.current = activeTabId;

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
    client
      .dispatchWorkspaceList({ directory: null })
      .then((response) => {
        const entries = extractWorkspaceListEntries(response).map(toInspectorEntry);
        setRootEntries(entries);
        setRootLoaded(true);
      })
      .catch((err) => {
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
            const entries = extractWorkspaceListEntries(response).map(toInspectorEntry);
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
          const children = extractWorkspaceListEntries(response).map(toInspectorEntry);
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
          logRef.current.append("info", "handshake accepted");
          void onHandshakeAck(client, {
            setPhase,
            setMessage,
            watchActiveRef,
            disposedRef: () => disposed,
            refreshRoot: () => refreshRootListing(client),
          });
        },
        onTransportOpen: () => {
          if (disposed) return;
          logRef.current.append("info", "transport open");
        },
        onTransportClosed: (reason) => {
          if (disposed) return;
          logRef.current.append(
            "warn",
            `transport closed: ${describeError(reason)}`,
          );
        },
        onWatchEvent: (_requestId: bigint, payload: unknown) => {
          if (disposed) return;
          refreshRootListing(client);
          refreshExpandedDirectories(client);
          const changed = extractChangedPaths(payload);
          const activeId = activeTabIdRef.current;
          const activeTab =
            openTabsRef.current.find((t) => t.id === activeId) ?? null;
          // Server-side watch currently reports directory-level changes (the
          // changed_paths list can be empty or hold the directory handle
          // only). To keep the auto-rerender behaviour useful we conservatively
          // bump the refresh signal whenever the active tab is a .scad and any
          // watch event fires; if a per-file payload arrives in the future the
          // pathKey match below can still narrow the trigger further.
          if (activeTab && activeTab.kind === "scad") {
            const activeKey = pathKey(activeTab.path);
            const matchedSpecific = changed.has(activeKey);
            setScadRefreshSignal((n) => n + 1);
            logRef.current.append(
              "info",
              matchedSpecific
                ? `auto rerender triggered by ${activeKey}`
                : `auto rerender triggered by ${activeKey} (directory change)`,
            );
          }
          for (const key of changed) {
            logRef.current.append("info", `watch event: ${key}`);
          }
        },
        onWatchResubscribed: () => {
          if (disposed) return;
          logRef.current.append("info", "watch resubscribed");
        },
      }),
    );
    clientRef.current = client;
    setClientReady(true);

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
    });
    transport.start();

    // bridge 契约 §6：tick 频率 ≥30Hz 或 requestAnimationFrame。
    // 用 rAF 跟着浏览器绘制节拍推进；tab 被隐藏时浏览器会自动降频。
    let rafHandle = 0;
    const rafTick = () => {
      if (disposed) return;
      client.pump();
      rafHandle = window.requestAnimationFrame(rafTick);
    };
    rafHandle = window.requestAnimationFrame(rafTick);

    return () => {
      disposed = true;
      window.cancelAnimationFrame(rafHandle);
      transport.stop();
      client.destroy();
      clientRef.current = null;
      setClientReady(false);
      watchActiveRef.current = false;
    };
  }, [wsUrl, refreshRootListing, refreshExpandedDirectories]);

  const entriesLoaded = rootLoaded;
  const entries: InspectorEntry[] = rootEntries;
  const rootName = snapshot?.workspace_current?.root_name ?? "(loading)";

  const handleInspectorOpen = useCallback(
    (entry: InspectorEntry) => {
      const kind = resolveTabKind(entry.label);
      if (!kind) {
        const ext = extensionOf(entry.label) || "(no extension)";
        setMessage(`unsupported file type: ${ext}`);
        return;
      }
      const id = pathKey(entry.path);
      openTab({ id, label: entry.label, path: entry.path, kind });
      setMessage(`opened ${entry.label}`);
    },
    [openTab],
  );

  const activeTab =
    openTabs.find((tab) => tab.id === activeTabId) ?? null;
  const previewTargetLabel = activeTab ? activeTab.label : "—";
  const client = clientReady ? clientRef.current : null;
  const meshSummary = meshStats
    ? { label: activeTab?.label ?? "mesh", ...meshStats }
    : null;
  const showMeshPanels = activeTab?.kind === "mesh";
  const defaultExportFilename = activeTab
    ? deriveExportFilename(activeTab.label)
    : "export.stl";

  return (
    <div className="app" data-testid="workbench-layout">
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
        previewTargetLabel={previewTargetLabel}
        tabs={openTabs}
        activeTabId={activeTabId}
        onActivateTab={setActiveTab}
        onCloseTab={closeTab}
        onPreviewStatus={setMessage}
        client={client}
        refreshSignal={scadRefreshSignal}
        onLog={log.append}
        meshStats={meshStats}
        activeView={activeView}
        onSelectView={setActiveView}
        onMeshStats={setMeshStats}
      />
      <Inspector
        rootName={rootName}
        entries={entries}
        entriesLoaded={entriesLoaded}
        onRequestPreview={handleInspectorOpen}
        onExpandDirectory={handleExpandDirectory}
        onCollapseDirectory={handleCollapseDirectory}
        previewTargetLabel={previewTargetLabel}
        meshSummary={meshSummary}
        expandedDirectories={expanded}
        directoryKey={pathKey}
        activeFilePath={activeTab ? activeTab.path : null}
        client={client}
        showMeshPanels={showMeshPanels}
        meshSource={activeTab?.path}
        defaultExportFilename={defaultExportFilename}
        onExportStatus={setMessage}
        bottomSlot={<LogPanel entries={log.entries} />}
      />
    </div>
  );
}

function deriveExportFilename(label: string): string {
  if (!label) return "export.stl";
  const idx = label.lastIndexOf(".");
  const stem = idx >= 0 ? label.slice(0, idx) : label;
  return `${stem || "export"}.stl`;
}

function extractChangedPaths(payload: unknown): Set<string> {
  const out = new Set<string>();
  if (!payload || typeof payload !== "object") return out;
  const outer = payload as Record<string, unknown>;
  // WatchEventPayload uses {type, payload} via serde(tag,content). Dig one
  // level before pulling changed_paths; fall back to the top level for
  // payload shapes we have not yet encountered.
  const inner =
    (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const changed = inner["changed_paths"] ?? outer["changed_paths"];
  if (!Array.isArray(changed)) return out;
  for (const item of changed) {
    if (!item || typeof item !== "object") continue;
    const segs = (item as Record<string, unknown>)["path_segments"];
    if (!Array.isArray(segs)) continue;
    const key = segs.filter((s): s is string => typeof s === "string").join("/");
    if (key.length > 0) out.add(key);
  }
  return out;
}

type HandshakeCtx = {
  setPhase: (value: Phase) => void;
  setMessage: (value: string) => void;
  watchActiveRef: React.MutableRefObject<boolean>;
  disposedRef: () => boolean;
  refreshRoot: () => void;
};

async function onHandshakeAck(
  client: WasmClient,
  ctx: HandshakeCtx,
): Promise<void> {
  try {
    await client.dispatchWorkspaceCurrent();
  } catch (err) {
    if (ctx.disposedRef()) return;
    ctx.setPhase("error");
    ctx.setMessage(`initial flow: ${describeError(err)}`);
    return;
  }
  if (ctx.disposedRef()) return;
  ctx.refreshRoot();
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

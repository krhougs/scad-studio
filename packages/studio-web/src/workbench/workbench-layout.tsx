// Workbench layout: CSS Grid 五区外框 + transport/protocol 生命周期接线。
// Phase 5 已接好 handshake / watch / left files panel / mesh_decode。
// Phase 6 补上文档标签系统（Tab Bar + DocumentTab Zustand 状态 + 多 viewer
// 挂载到 Canvas Zone）。左栏 Files 点击文件 → 按扩展名路由到对应 viewer tab；
// 不支持的扩展名仅更新状态条消息，不开 tab。
//
// 协议业务状态仍在 wasm 内；Zustand 只存 UI 壳状态（openTabs / activeTabId
// / sidePanelOpen 等）。viewer 自己发 FileRead / PreviewRequest，tab 只记
// id / label / path / kind。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  decodeConfigLoad,
  describeConfigGaps,
} from "../config/app-config";
import {
  setAppConfigError,
  setAppConfigLoading,
  setAppConfigReady,
  useAppConfigState,
} from "../config/app-config-store";
import { WasmClient } from "../wasm-bridge";
import { useUiStore } from "../state/ui-store";
import { CanvasZone, type ViewPreset } from "./canvas-zone";
import { documentTitleForFile } from "./document-title";
import { Inspector } from "./inspector";
import { LeftPanel } from "./left-panel";
import {
  LEFT_PANEL_PARAM,
  normalizeLeftPanelId,
} from "./left-panel-routing";
import { useLogBuffer } from "./use-log-buffer";
import { pathKey, pathLabel } from "./path-utils";
import { Rail } from "./rail";
import {
  scadInspectorPanelsForState,
  useScadWorkbenchState,
} from "./scad-workbench";
import { resolveTabKind, extensionOf } from "./tab-kind";
import { Topbar, type TopbarStatus } from "./topbar";
import type { WorkspaceDirectoryNode, WorkspaceEntry } from "./workspace-tree";
import {
  createTransport,
  describeError,
  buildClientCallbacks,
} from "./workbench-wiring";
import { describeFileReadError } from "../viewers/file-read-decoder";

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

function resolveWsUrl(searchParams: URLSearchParams): string {
  const fromQuery = searchParams.get("ws");
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

function toWorkspaceEntry(entry: ProtocolEntry): WorkspaceEntry {
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
  const [searchParams, setSearchParams] = useSearchParams();
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string>("");
  const [snapshot, setSnapshot] = useState<Snapshot>(null);
  const [expanded, setExpanded] = useState<Map<string, WorkspaceDirectoryNode>>(
    () => new Map(),
  );
  const [rootEntries, setRootEntries] = useState<WorkspaceEntry[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [clientReady, setClientReady] = useState(false);
  const [activeView, setActiveView] = useState<ViewPreset>("iso");
  const [meshStats, setMeshStats] = useState<
    { vertices: number; indices: number } | null
  >(null);
  const [activeDefines, setActiveDefines] = useState<string[]>([]);

  const openTabs = useUiStore((s) => s.openTabs);
  const activeTabId = useUiStore((s) => s.activeTabId);
  const openTab = useUiStore((s) => s.openTab);
  const closeTab = useUiStore((s) => s.closeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const activeRail = useUiStore((s) => s.activeRail);
  const setActiveRail = useUiStore((s) => s.setActiveRail);

  const wsUrl = useMemo(() => resolveWsUrl(searchParams), [searchParams]);
  const clientRef = useRef<WasmClient | null>(null);
  const expandedRef = useRef<Map<string, WorkspaceDirectoryNode>>(new Map());
  const watchActiveRef = useRef(false);
  const log = useLogBuffer();
  const logRef = useRef(log);
  logRef.current = log;
  const [documentRefreshSignal, setDocumentRefreshSignal] = useState(0);
  const openTabsRef = useRef(openTabs);
  openTabsRef.current = openTabs;
  const activeTabIdRef = useRef(activeTabId);
  activeTabIdRef.current = activeTabId;
  const appConfig = useAppConfigState();
  const routePanelValue = searchParams.get(LEFT_PANEL_PARAM);
  const routePanel = normalizeLeftPanelId(routePanelValue);
  const activeTab =
    openTabs.find((tab) => tab.id === activeTabId) ?? null;
  const client = clientReady ? clientRef.current : null;
  const scadWorkbenchState = useScadWorkbenchState({
    path: activeTab?.kind === "scad" ? activeTab.path : null,
    client,
    refreshSignal: documentRefreshSignal,
    onLog: log.append,
    onPreviewStatus: setMessage,
    enabled: activeTab?.kind === "scad" && client !== null,
  });

  useEffect(() => {
    if (activeRail !== routePanel) {
      setActiveRail(routePanel);
    }
    if (routePanelValue !== routePanel) {
      setSearchParams((prev) => {
        prev.set(LEFT_PANEL_PARAM, routePanel);
        return prev;
      }, {
        replace: true,
      });
    }
  }, [
    activeRail,
    routePanel,
    routePanelValue,
    setActiveRail,
    setSearchParams,
  ]);

  useEffect(() => {
    if (activeTab?.kind === "scad") {
      setActiveDefines(scadWorkbenchState.appliedDefines);
      return;
    }
    setActiveDefines([]);
  }, [activeTab?.kind, scadWorkbenchState.appliedDefines]);

  const setExpandedBoth = useCallback(
    (
      updater: (
        prev: Map<string, WorkspaceDirectoryNode>,
      ) => Map<string, WorkspaceDirectoryNode>,
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
        const entries = extractWorkspaceListEntries(response).map(toWorkspaceEntry);
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
            const entries = extractWorkspaceListEntries(response).map(toWorkspaceEntry);
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
    (entry: WorkspaceEntry) => {
      const client = clientRef.current;
      if (!client) return;
      const key = pathKey(entry.path);
      const pending: WorkspaceDirectoryNode = {
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
          const children = extractWorkspaceListEntries(response).map(toWorkspaceEntry);
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
    (entry: WorkspaceEntry) => {
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

  const refreshAppConfig = useCallback((client: WasmClient) => {
    setAppConfigLoading();
    client
      .dispatchConfigLoad()
      .then((response) => {
        const decoded = decodeConfigLoad(response);
        setAppConfigReady(decoded.config, decoded.raw, "load");
        const gaps = describeConfigGaps(decoded.config);
        if (gaps.length > 0) {
          logRef.current.append("warn", `config incomplete: ${gaps.join(", ")}`);
        } else {
          logRef.current.append("info", "config loaded");
        }
      })
      .catch((err) => {
        const message = describeFileReadError(err);
        setAppConfigError(message);
        logRef.current.append("warn", `config load failed: ${message}`);
      });
  }, []);

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
          }).then(() => {
            if (disposed) return;
            refreshAppConfig(client);
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
          if (
            activeTab &&
            (activeTab.kind === "scad" ||
              activeTab.kind === "mesh" ||
              activeTab.kind === "markdown" ||
              activeTab.kind === "image")
          ) {
            const activeKey = pathKey(activeTab.path);
            const matchedSpecific = changed.has(activeKey);
            setDocumentRefreshSignal((n) => n + 1);
            logRef.current.append(
              "info",
              matchedSpecific
                ? `document refresh triggered by ${activeKey}`
                : `document refresh triggered by ${activeKey} (directory change)`,
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
  }, [refreshAppConfig, refreshExpandedDirectories, refreshRootListing, wsUrl]);

  const entriesLoaded = rootLoaded;
  const entries: WorkspaceEntry[] = rootEntries;
  const rootName = snapshot?.workspace_current?.root_name ?? "(loading)";

  const handleOpenEntry = useCallback(
    (entry: WorkspaceEntry) => {
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

  const previewTargetLabel = activeTab ? activeTab.label : "—";
  const meshSummary = meshStats
    ? { label: activeTab?.label ?? "mesh", ...meshStats }
    : null;
  const showMeshPanels = activeTab?.kind === "mesh" || activeTab?.kind === "scad";
  const defaultExportFilename = activeTab
    ? deriveExportFilename(activeTab.label)
    : "export.stl";

  useEffect(() => {
    document.title = documentTitleForFile(activeTab?.label ?? null);
  }, [activeTab?.label]);

  const scadInspectorPanels =
    activeTab?.kind === "scad"
      ? scadInspectorPanelsForState(scadWorkbenchState)
      : null;

  return (
    <div className="app" data-testid="workbench-layout">
      <Topbar
        workspaceName={rootName}
        wsUrl={wsUrl}
        status={phaseToStatus(phase)}
        message={message}
      />
      <Rail />
      <LeftPanel
        activePanel={routePanel}
        rootName={rootName}
        entries={entries}
        entriesLoaded={entriesLoaded}
        activeFilePath={activeTab ? activeTab.path : null}
        expandedDirectories={expanded}
        directoryKey={pathKey}
        onRequestPreview={handleOpenEntry}
        onExpandDirectory={handleExpandDirectory}
        onCollapseDirectory={handleCollapseDirectory}
        logEntries={log.entries}
        client={client}
        appConfig={appConfig}
        wsUrl={wsUrl}
      />
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
        refreshSignal={documentRefreshSignal}
        config={appConfig.kind === "ready" ? appConfig.config : null}
        meshStats={meshStats}
        activeView={activeView}
        onSelectView={setActiveView}
        onMeshStats={setMeshStats}
        scadWorkbenchState={scadWorkbenchState}
      />
      <Inspector
        rootName={rootName}
        previewTargetLabel={previewTargetLabel}
        meshSummary={meshSummary}
        client={client}
        showMeshPanels={showMeshPanels}
        meshSource={activeTab?.path}
        defaultExportFilename={defaultExportFilename}
        exportDefines={activeTab?.kind === "scad" ? activeDefines : []}
        appConfig={appConfig}
        onExportStatus={setMessage}
        parametersSlot={
          activeTab?.kind === "scad" ? scadInspectorPanels?.parameters : null
        }
        presetsSlot={
          activeTab?.kind === "scad" ? scadInspectorPanels?.presets : null
        }
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

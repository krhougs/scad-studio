// Workbench layout: CSS Grid 五区外框 + transport/protocol 生命周期接线。
// Phase 5 已接好 handshake / watch / left files panel / mesh_decode。
// Phase 6 补上文档标签系统（Tab Bar + DocumentTab Zustand 状态 + 多 viewer
// 挂载到 Canvas Zone）。左栏 Files 点击文件 → 按扩展名路由到对应 viewer tab；
// 不支持的扩展名仅更新状态条消息，不开 tab。
//
// 协议业务状态通过 protocol-store (Zustand) 按域拆分订阅，避免全树
// re-render。UI 壳状态在 ui-store。viewer 自己发 FileRead /
// PreviewRequest，tab 只记 id / label / path / kind。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  decodeConfigLoad,
  describeConfigGaps,
  encodeConfigRaw,
  normalizeAppConfig,
  toConfigSaveRequest,
} from "../config/app-config";
import {
  setAppConfigError,
  setAppConfigLoading,
  setAppConfigReady,
  useAppConfigState,
} from "../config/app-config-store";
import { WasmClient } from "../wasm-bridge";
import { useUiStore } from "../state/ui-store";
import {
  useProtocolStore,
  useWorkspaceName,
  useAgentRun,
  useChatSessions,
  useCurrentChatSession,
} from "../state/protocol-store";
import { CanvasZone, type ViewPreset } from "./canvas-zone";
import { runSavedPlan } from "./chat-actions";
import type { CameraState } from "../canvas/camera-state";
import { CameraInspector } from "./camera-inspector";
import {
  extractCadQueryReadyFromAgentEvent,
} from "./cadquery-result-tab";
import {
  cadQueryArtifactTabPathForFile,
  cadQueryTabMatchesReady,
  isCadQueryStepFile,
} from "./cadquery-source-path";
import { CadQueryRefTree } from "./cadquery-ref-tree";
import { documentTitleForFile } from "./document-title";
import { Inspector } from "./inspector";
import { LeftPanel } from "./left-panel";
import { LEFT_PANEL_PARAM, normalizeLeftPanelId } from "./left-panel-routing";
import { useLogBuffer } from "./use-log-buffer";
import { pathKey, pathLabel } from "./path-utils";
import { Rail } from "./rail";
import {
  scadInspectorPanelsForState,
  useScadWorkbenchState,
} from "./scad-workbench";
import { derivePresetPath } from "./preset-io";
import { resolveTabKind, extensionOf } from "./tab-kind";
import { Topbar, type TopbarStatus } from "./topbar";
import type { MeshInfo } from "../viewers/mesh-info";
import type { CadQueryScenePayload } from "../viewers/cadquery-mesh";
import type { PlanRunTarget } from "../viewers/plan-preview-path";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";
import type { WorkspaceDirectoryNode, WorkspaceEntry } from "./workspace-tree";
import {
  createTransport,
  describeError,
  buildClientCallbacks,
} from "./workbench-wiring";
import { describeFileReadError } from "../viewers/file-read-decoder";
import { resolveWorkbenchWsUrl } from "./ws-url";
import {
  shouldRefreshDocumentForWatch,
  shouldRefreshScadSettingsForWatch,
} from "./watch-refresh";

type Phase = "idle" | "connecting" | "handshaking" | "ready" | "error";

type ProtocolEntry = {
  name?: unknown;
  path: unknown;
  kind: "directory" | "file";
  path_error?: unknown;
};


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
  const pathError =
    typeof entry.path_error === "string" ? entry.path_error : null;
  const hasPath = entry.path !== null && entry.path !== undefined;
  return {
    label:
      typeof entry.name === "string" && entry.name.length > 0
        ? entry.name
        : pathLabel(entry.path) || "(unnamed)",
    path: entry.path,
    kind: entry.kind,
    pathError,
    isOperable: hasPath && !pathError,
  };
}

function extractWorkspaceListEntries(response: unknown): ProtocolEntry[] {
  if (!response || typeof response !== "object") return [];
  const outer = response as Record<string, unknown>;
  const inner =
    (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const entries = (inner["entries"] as ProtocolEntry[] | undefined) ?? [];
  return Array.isArray(entries) ? entries : [];
}

export function WorkbenchLayout() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState<string>("");
  const applySnapshot = useProtocolStore((s) => s.applySnapshot);
  const rootName = useWorkspaceName();
  const agentRun = useAgentRun();
  const chatSessions = useChatSessions();
  const currentChatSession = useCurrentChatSession();
  const currentSelection = useProtocolStore((s) => s.current_selection);
  const cadQueryResults = useProtocolStore((s) => s.cadquery_results);
  const [expanded, setExpanded] = useState<Map<string, WorkspaceDirectoryNode>>(
    () => new Map(),
  );
  const [rootEntries, setRootEntries] = useState<WorkspaceEntry[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [clientReady, setClientReady] = useState(false);
  const activeView: ViewPreset = "iso";
  const [meshInfo, setMeshInfo] = useState<MeshInfo | null>(null);
  const [cadQueryScene, setCadQueryScene] =
    useState<CadQueryScenePayload | null>(null);
  const [cameraState, setCameraState] = useState<CameraState | null>(null);
  const [cameraOverride, setCameraOverride] = useState<CameraState | null>(
    null,
  );
  const [markdownPlanBusy, setMarkdownPlanBusy] = useState(false);
  const [activeDefines, setActiveDefines] = useState<string[]>([]);
  const [panelWidths, setPanelWidths] = useState({
    left: 360,
    right: 320,
  });

  const openTabs = useUiStore((s) => s.openTabs);
  const activeTabId = useUiStore((s) => s.activeTabId);
  const openTab = useUiStore((s) => s.openTab);
  const closeTab = useUiStore((s) => s.closeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const activeRail = useUiStore((s) => s.activeRail);
  const setActiveRail = useUiStore((s) => s.setActiveRail);

  const wsUrl = useMemo(
    () =>
      resolveWorkbenchWsUrl(searchParams, {
        envUrl: import.meta.env.VITE_WS_URL,
      }),
    [searchParams],
  );
  const clientRef = useRef<WasmClient | null>(null);
  const applySnapshotRef = useRef(applySnapshot);
  applySnapshotRef.current = applySnapshot;
  const expandedRef = useRef<Map<string, WorkspaceDirectoryNode>>(new Map());
  const watchActiveRef = useRef(false);
  const log = useLogBuffer();
  const logRef = useRef(log);
  logRef.current = log;
  const [documentRefreshSignal, setDocumentRefreshSignal] = useState(0);
  const [scadSettingsRefreshSignal, setScadSettingsRefreshSignal] = useState(0);
  const openTabsRef = useRef(openTabs);
  openTabsRef.current = openTabs;
  const activeTabIdRef = useRef(activeTabId);
  activeTabIdRef.current = activeTabId;
  const appConfig = useAppConfigState();
  const routePanelValue = searchParams.get(LEFT_PANEL_PARAM);
  const routePanel = normalizeLeftPanelId(routePanelValue);
  const activeTab = openTabs.find((tab) => tab.id === activeTabId) ?? null;
  const client = clientReady ? clientRef.current : null;
  const scadWorkbenchState = useScadWorkbenchState({
    path: activeTab?.kind === "scad" ? activeTab.path : null,
    client,
    refreshSignal: documentRefreshSignal,
    settingsRefreshSignal: scadSettingsRefreshSignal,
    onLog: log.append,
    onPreviewStatus: setMessage,
    enabled: activeTab?.kind === "scad" && client !== null,
  });

  useEffect(() => {
    if (appConfig.kind !== "ready") return;
    setPanelWidths({
      left: appConfig.config.left_panel_width ?? 360,
      right: appConfig.config.right_panel_width ?? 320,
    });
  }, [appConfig]);

  const persistPanelWidths = useCallback(
    (next: { left: number; right: number }) => {
      if (!client || appConfig.kind !== "ready") return;
      const config = normalizeAppConfig({
        ...appConfig.config,
        left_panel_width: next.left,
        right_panel_width: next.right,
      });
      const raw = encodeConfigRaw(config);
      client
        .dispatchConfigSave(toConfigSaveRequest(config))
        .then(() => setAppConfigReady(config, raw, "save"))
        .catch((err) => {
          logRef.current.append(
            "warn",
            `panel width save failed: ${describeFileReadError(err)}`,
          );
        });
    },
    [appConfig, client],
  );

  const beginPanelResize = useCallback(
    (side: "left" | "right", event: React.PointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      const startX = event.clientX;
      const startWidth = panelWidths[side];
      let latest = startWidth;
      const onMove = (move: PointerEvent) => {
        const delta = move.clientX - startX;
        latest = clampPanelWidth(
          side === "left" ? startWidth + delta : startWidth - delta,
        );
        setPanelWidths((prev) => ({ ...prev, [side]: latest }));
      };
      const onUp = () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        const next = { ...panelWidths, [side]: latest };
        setPanelWidths(next);
        persistPanelWidths(next);
      };
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp, { once: true });
    },
    [panelWidths, persistPanelWidths],
  );

  useEffect(() => {
    if (activeRail !== routePanel) {
      setActiveRail(routePanel);
    }
    if (routePanelValue !== routePanel) {
      setSearchParams(
        (prev) => {
          prev.set(LEFT_PANEL_PARAM, routePanel);
          return prev;
        },
        {
          replace: true,
        },
      );
    }
  }, [activeRail, routePanel, routePanelValue, setActiveRail, setSearchParams]);

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
        const entries =
          extractWorkspaceListEntries(response).map(toWorkspaceEntry);
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
            const entries =
              extractWorkspaceListEntries(response).map(toWorkspaceEntry);
            setExpandedBoth((prev) => {
              const next = new Map(prev);
              next.set(key, { ...state, entries, loading: false, error: null });
              return next;
            });
          })
          .catch((err) => {
            setExpandedBoth((prev) => {
              const next = new Map(prev);
              next.set(key, {
                ...state,
                loading: false,
                error: describeError(err),
              });
              return next;
            });
          });
      }
    },
    [setExpandedBoth],
  );

  const handleRefreshFiles = useCallback(() => {
    const client = clientRef.current;
    if (!client) return;
    refreshRootListing(client);
    refreshExpandedDirectories(client);
  }, [refreshExpandedDirectories, refreshRootListing]);

  const handleExpandDirectory = useCallback(
    (entry: WorkspaceEntry) => {
      if (entry.isOperable === false || entry.pathError) return;
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
          const children =
            extractWorkspaceListEntries(response).map(toWorkspaceEntry);
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
          logRef.current.append(
            "warn",
            `config incomplete: ${gaps.join(", ")}`,
          );
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
          applySnapshotRef.current(client.snapshot());
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
          if (activeTab) {
            const activeKey = pathKey(activeTab.path);
            const activeSettingsKey =
              activeTab.kind === "scad"
                ? pathKey(derivePresetPath(activeTab.path))
                : "";
            const matchedSettings =
              activeSettingsKey.length > 0 && changed.has(activeSettingsKey);
            if (shouldRefreshDocumentForWatch(activeTab, changed, matchedSettings)) {
              setDocumentRefreshSignal((n) => n + 1);
              logRef.current.append(
                "info",
                `document refresh triggered by ${activeKey}`,
              );
            }
            if (
              shouldRefreshScadSettingsForWatch(
                activeTab,
                changed,
                matchedSettings,
              )
            ) {
              setScadSettingsRefreshSignal((n) => n + 1);
              const source = matchedSettings
                ? activeSettingsKey
                : "directory change";
              logRef.current.append(
                "info",
                `scad settings refresh triggered by ${source}`,
              );
            }
          }
          for (const key of changed) {
            logRef.current.append("info", `watch event: ${key}`);
          }
        },
        onWatchResubscribed: () => {
          if (disposed) return;
          logRef.current.append("info", "watch resubscribed");
        },
        onAgentEvent: (payload) => {
          if (disposed) return;
          const ready = extractCadQueryReadyFromAgentEvent(payload);
          if (ready) {
            const activeId = activeTabIdRef.current;
            const activeTab =
              openTabsRef.current.find((tab) => tab.id === activeId) ?? null;
            if (
              activeTab?.kind === "cadquery" &&
              cadQueryTabMatchesReady(activeTab.path, ready)
            ) {
              setDocumentRefreshSignal((n) => n + 1);
            }
            setMessage(`cadquery ready ${ready.result_id}`);
          }
          const eventName = describeAgentEvent(payload);
          logRef.current.append("info", `agent event: ${eventName}`);
          if (eventName === "error") {
            const p = payload as Record<string, unknown>;
            console.error(
              `[agent error] ${p["error_type"] ?? "unknown"}: ${p["message"] ?? "(no message)"}`,
            );
          }
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

  const handleOpenPath = useCallback(
    (path: unknown, label = pathLabel(path)) => {
      if (isCadQueryStepFile(label)) {
        const artifactPath = cadQueryArtifactTabPathForFile(
          path,
          cadQueryResults,
        );
        if (!artifactPath) {
          setMessage(`unsupported file type: ${extensionOf(label)}`);
          return;
        }
        openTab({
          id: `cadquery-artifact:${pathKey(path)}`,
          label,
          path: artifactPath,
          kind: "cadquery",
        });
        setMessage(`opened ${label}`);
        return;
      }
      const kind = resolveTabKind(label);
      if (!kind) {
        const ext = extensionOf(label) || "(no extension)";
        setMessage(`unsupported file type: ${ext}`);
        return;
      }
      const id = pathKey(path);
      openTab({ id, label, path, kind });
      setMessage(`opened ${label}`);
    },
    [cadQueryResults, openTab],
  );

  const handleOpenEntry = useCallback(
    (entry: WorkspaceEntry) => {
      if (entry.isOperable === false || entry.pathError) {
        setMessage(`invalid workspace entry: ${entry.label}`);
        return;
      }
      handleOpenPath(entry.path, entry.label);
    },
    [handleOpenPath],
  );

  const handleRunMarkdownPlan = useCallback(
    (target: PlanRunTarget) => {
      const activeClient = clientRef.current;
      if (!activeClient || markdownPlanBusy || agentRun) return;
      void runSavedPlan({
        client: activeClient,
        planId: target.planId,
        planRef: target.planRef,
        currentSessionId: currentChatSession,
        sessions: chatSessions,
        agentRun,
        busy: markdownPlanBusy,
        contextPills: [],
        onStatus: setMessage,
        setBusy: setMarkdownPlanBusy,
      });
    },
    [
      agentRun,
      markdownPlanBusy,
      chatSessions,
      currentChatSession,
    ],
  );

  const previewTargetLabel = activeTab ? activeTab.label : "—";
  const meshSummary = meshInfo
    ? { label: activeTab?.label ?? "mesh", ...meshInfo }
    : null;
  const showMeshPanels =
    activeTab?.kind === "mesh" ||
    activeTab?.kind === "scad" ||
    activeTab?.kind === "cadquery";
  const defaultExportFilename = activeTab
    ? deriveExportFilename(activeTab.label)
    : "export.stl";

  useEffect(() => {
    document.title = documentTitleForFile(activeTab?.label ?? null);
  }, [activeTab?.label]);

  useEffect(() => {
    setCameraOverride(null);
    setCameraState(null);
  }, [activeTab?.kind, activeTab?.path]);

  useEffect(() => {
    if (activeTab?.kind !== "cadquery") setCadQueryScene(null);
  }, [activeTab?.kind, activeTab?.path]);

  const handleCadQuerySelectionChange = useCallback(
    (next: SelectionUpdateRequest) => {
      void clientRef.current?.dispatchSelectionUpdate(next);
    },
    [],
  );

  const scadInspectorPanels =
    activeTab?.kind === "scad"
      ? scadInspectorPanelsForState(scadWorkbenchState)
      : null;

  return (
    <div
      className="app"
      data-testid="workbench-layout"
      style={
        {
          "--left-panel-width": `${panelWidths.left}px`,
          "--right-panel-width": `${panelWidths.right}px`,
        } as React.CSSProperties
      }
    >
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
        onOpenPath={handleOpenPath}
        onExpandDirectory={handleExpandDirectory}
        onCollapseDirectory={handleCollapseDirectory}
        onRefreshFiles={handleRefreshFiles}
        logEntries={log.entries}
        client={client}
        onStatus={setMessage}
        appConfig={appConfig}
        wsUrl={wsUrl}
      />
      <PanelResizeHandle side="left" onPointerDown={beginPanelResize} />
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
        meshInfo={meshInfo}
        activeView={activeView}
        onMeshInfo={setMeshInfo}
        onCadQueryScene={setCadQueryScene}
        cadQueryScene={cadQueryScene}
        cadQuerySelection={currentSelection}
        cameraState={cameraState}
        cameraOverride={cameraOverride}
        onCameraChange={setCameraState}
        scadWorkbenchState={scadWorkbenchState}
        planRunDisabled={!client || markdownPlanBusy || Boolean(agentRun)}
        onRunPlan={handleRunMarkdownPlan}
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
        refTreeSlot={
          activeTab?.kind === "cadquery" ? (
            <CadQueryRefTree
              scene={cadQueryScene}
              selection={currentSelection}
              onSelectionChange={handleCadQuerySelectionChange}
            />
          ) : null
        }
        cameraSlot={
          showMeshPanels ? (
            <CameraInspector
              camera={cameraState}
              meshInfo={meshInfo}
              onChange={(camera) => {
                setCameraState(camera);
                setCameraOverride(camera);
              }}
            />
          ) : null
        }
        parametersSlot={
          activeTab?.kind === "scad" ? scadInspectorPanels?.parameters : null
        }
        appearanceSlot={
          activeTab?.kind === "scad" ? scadInspectorPanels?.appearance : null
        }
        presetsSlot={
          activeTab?.kind === "scad" ? scadInspectorPanels?.presets : null
        }
      />
      <PanelResizeHandle side="right" onPointerDown={beginPanelResize} />
    </div>
  );
}

function describeAgentEvent(payload: unknown): string {
  if (!payload || typeof payload !== "object") return "unknown";
  const event = (payload as Record<string, unknown>)["event"];
  return typeof event === "string" ? event : "unknown";
}

function PanelResizeHandle({
  side,
  onPointerDown,
}: {
  side: "left" | "right";
  onPointerDown: (
    side: "left" | "right",
    event: React.PointerEvent<HTMLDivElement>,
  ) => void;
}) {
  return (
    <div
      className={`panel-resize-handle panel-resize-handle--${side}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={`${side} panel width`}
      data-testid={`resize-${side}-panel`}
      onPointerDown={(event) => onPointerDown(side, event)}
    />
  );
}

function clampPanelWidth(value: number): number {
  return Math.min(640, Math.max(280, Math.round(value)));
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
    const key = segs
      .filter((s): s is string => typeof s === "string")
      .join("/");
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
  client
    .dispatchChatList({ include_archived: false })
    .catch((err) => {
      ctx.setMessage(`chat list failed: ${describeError(err)}`);
    });
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

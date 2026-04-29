import type { AppConfigState } from "../config/app-config";
import type { WasmClient } from "../wasm-bridge";
import { ChatZone } from "./chat-zone";
import type { ChatSnapshot } from "./chat-zone";
import { FilesPanel } from "./files-panel";
import type { LeftPanelId } from "./left-panel-routing";
import { LogPanel, type LogEntry } from "./log-panel";
import { SettingsPanel } from "./settings-panel";
import { SidePanelHeader } from "./side-panel-header";
import type { WorkspaceDirectoryNode, WorkspaceEntry } from "./workspace-tree";

type LeftPanelProps = {
  activePanel: LeftPanelId;
  rootName: string;
  entries: WorkspaceEntry[];
  entriesLoaded: boolean;
  activeFilePath: unknown | null;
  expandedDirectories: Map<string, WorkspaceDirectoryNode>;
  directoryKey: (path: unknown) => string;
  onRequestPreview: (entry: WorkspaceEntry) => void;
  onOpenPath: (path: unknown) => void;
  onExpandDirectory: (entry: WorkspaceEntry) => void;
  onCollapseDirectory: (entry: WorkspaceEntry) => void;
  logEntries: LogEntry[];
  client: WasmClient | null;
  snapshot: ChatSnapshot | null;
  onStatus?: (message: string) => void;
  appConfig: AppConfigState;
  wsUrl: string;
};

export function LeftPanel(props: LeftPanelProps) {
  const {
    activePanel,
    rootName,
    entries,
    entriesLoaded,
    activeFilePath,
    expandedDirectories,
    directoryKey,
    onRequestPreview,
    onOpenPath,
    onExpandDirectory,
    onCollapseDirectory,
    logEntries,
    client,
    snapshot,
    onStatus,
    appConfig,
    wsUrl,
  } = props;

  return (
    <aside
      className="workbench-left-panel"
      data-testid="workbench-left-panel"
      aria-label="left panel"
    >
      {activePanel === "chat" ? (
        <ChatZone
          client={client}
          snapshot={snapshot}
          onStatus={onStatus}
          onOpenPlan={onOpenPath}
        />
      ) : null}
      {activePanel === "files" ? (
        <FilesPanel
          rootName={rootName}
          entries={entries}
          entriesLoaded={entriesLoaded}
          activeFilePath={activeFilePath}
          expandedDirectories={expandedDirectories}
          directoryKey={directoryKey}
          onRequestPreview={onRequestPreview}
          onExpandDirectory={onExpandDirectory}
          onCollapseDirectory={onCollapseDirectory}
        />
      ) : null}
      {activePanel === "settings" ? (
        <SettingsPanel client={client} appConfig={appConfig} wsUrl={wsUrl} />
      ) : null}
      {activePanel === "log" ? (
        <section
          className="side-panel side-panel--log side-panel--flush"
          data-testid="left-panel-log"
          aria-label="log"
        >
          <SidePanelHeader title="log" meta={`${logEntries.length} entries`} />
          <div className="side-panel__body" data-testid="log-panel">
            <LogPanel entries={logEntries} />
          </div>
        </section>
      ) : null}
      {["parts", "materials", "queue", "history"].includes(activePanel) ? (
        <PlaceholderPanel id={activePanel} />
      ) : null}
    </aside>
  );
}

function PlaceholderPanel({ id }: { id: string }) {
  return (
    <section className="side-panel" data-testid={`left-panel-${id}`}>
      <SidePanelHeader title={id} meta="not connected" />
      <div className="side-panel__body">
        <p className="side-panel__empty">panel placeholder</p>
      </div>
    </section>
  );
}

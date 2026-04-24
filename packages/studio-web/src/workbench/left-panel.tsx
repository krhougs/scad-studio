import type { AppConfigState } from "../config/app-config";
import type { WasmClient } from "../wasm-bridge";
import { ChatZone } from "./chat-zone";
import { FilesPanel } from "./files-panel";
import type { LeftPanelId } from "./left-panel-routing";
import { LogPanel, type LogEntry } from "./log-panel";
import { SettingsPanel } from "./settings-panel";
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
  onExpandDirectory: (entry: WorkspaceEntry) => void;
  onCollapseDirectory: (entry: WorkspaceEntry) => void;
  logEntries: LogEntry[];
  client: WasmClient | null;
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
    onExpandDirectory,
    onCollapseDirectory,
    logEntries,
    client,
    appConfig,
    wsUrl,
  } = props;

  return (
    <aside
      className="workbench-left-panel"
      data-testid="workbench-left-panel"
      aria-label="left panel"
    >
      {activePanel === "chat" ? <ChatZone /> : null}
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
          className="side-panel side-panel--log"
          data-testid="left-panel-log"
          aria-label="log"
        >
          <header className="side-panel__head">
            <div>
              <div className="title">§ log</div>
              <div className="sub">runtime events</div>
            </div>
          </header>
          <div className="side-panel__body">
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
      <header className="side-panel__head">
        <div>
          <div className="title">§ {id}</div>
          <div className="sub">not connected</div>
        </div>
      </header>
      <div className="side-panel__body">
        <p className="side-panel__empty">panel placeholder</p>
      </div>
    </section>
  );
}

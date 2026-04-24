import { WorkspaceTree, type WorkspaceDirectoryNode, type WorkspaceEntry } from "./workspace-tree";
import { SidePanelHeader } from "./side-panel-header";

type FilesPanelProps = {
  rootName: string;
  entries: WorkspaceEntry[];
  entriesLoaded: boolean;
  activeFilePath: unknown | null;
  expandedDirectories: Map<string, WorkspaceDirectoryNode>;
  directoryKey: (path: unknown) => string;
  onRequestPreview: (entry: WorkspaceEntry) => void;
  onExpandDirectory: (entry: WorkspaceEntry) => void;
  onCollapseDirectory: (entry: WorkspaceEntry) => void;
};

export function FilesPanel(props: FilesPanelProps) {
  const {
    rootName,
    entries,
    entriesLoaded,
    activeFilePath,
    expandedDirectories,
    directoryKey,
    onRequestPreview,
    onExpandDirectory,
    onCollapseDirectory,
  } = props;
  return (
    <section
      className="side-panel side-panel--files side-panel--flush"
      data-testid="left-panel-files"
      aria-label="files"
    >
      <SidePanelHeader title="files" meta={rootName} />
      <div className="side-panel__body">
        {!entriesLoaded ? (
          <div className="tree">
            <div className="tree-loading">entries loading…</div>
          </div>
        ) : entries.length === 0 ? (
          <div className="tree">
            <div className="tree-empty">workspace is empty</div>
          </div>
        ) : (
          <WorkspaceTree
            entries={entries}
            activeFilePath={activeFilePath}
            expandedDirectories={expandedDirectories}
            directoryKey={directoryKey}
            onRequestPreview={onRequestPreview}
            onExpandDirectory={onExpandDirectory}
            onCollapseDirectory={onCollapseDirectory}
          />
        )}
      </div>
    </section>
  );
}

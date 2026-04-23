// Inspector: workspace 目录条目树 + 选中文件元数据。Phase 5 补齐：
//   - 递归展开/折叠 directory entry（点击触发父级传入的 onExpandDirectory，
//     由 WorkbenchLayout 发起 `dispatch_workspace_list({ directory })`）。
//   - 点击 file 保留现有 preview 触发入口。
//   - 预览就绪后展示 mesh 解码元数据（vertices / indices）。

export type InspectorEntry = {
  label: string;
  path: unknown;
  kind: "file" | "directory";
};

export type InspectorDirectoryNode = {
  key: string;
  label: string;
  path: unknown;
  entries: InspectorEntry[] | null;
  loading: boolean;
  error: string | null;
};

export type InspectorMeshSummary = {
  label: string;
  vertices: number;
  indices: number;
};

type InspectorProps = {
  rootName: string;
  entries: InspectorEntry[];
  entriesLoaded: boolean;
  onRequestPreview: (entry: InspectorEntry) => void;
  onExpandDirectory: (entry: InspectorEntry) => void;
  onCollapseDirectory: (entry: InspectorEntry) => void;
  previewTargetLabel: string;
  meshSummary: InspectorMeshSummary | null;
  expandedDirectories: Map<string, InspectorDirectoryNode>;
  directoryKey: (path: unknown) => string;
};

export function Inspector({
  rootName,
  entries,
  entriesLoaded,
  onRequestPreview,
  onExpandDirectory,
  onCollapseDirectory,
  previewTargetLabel,
  meshSummary,
  expandedDirectories,
  directoryKey,
}: InspectorProps) {
  return (
    <aside
      className="workbench__inspector"
      aria-label="inspector"
      data-testid="workbench-inspector"
    >
      <header className="inspector__head">
        <div className="inspector__kicker">§ inspector</div>
        <div className="inspector__title">{rootName}</div>
      </header>
      <div className="inspector__body">
        <section className="inspector__section" data-testid="inspector-entries">
          <h5>entries</h5>
          {!entriesLoaded ? (
            <p className="chat__placeholder">workspace entries loading…</p>
          ) : entries.length === 0 ? (
            <p className="chat__placeholder">workspace is empty.</p>
          ) : (
            <ul
              className="inspector__list"
              data-testid="entries"
              role="tree"
            >
              {entries.map((entry) => (
                <EntryNode
                  key={`${entry.label}-${directoryKey(entry.path)}`}
                  entry={entry}
                  depth={0}
                  onRequestPreview={onRequestPreview}
                  onExpandDirectory={onExpandDirectory}
                  onCollapseDirectory={onCollapseDirectory}
                  expandedDirectories={expandedDirectories}
                  directoryKey={directoryKey}
                />
              ))}
            </ul>
          )}
        </section>
        <section className="inspector__section">
          <h5>preview target</h5>
          <div className="inspector__field">
            <span className="inspector__field-label">active</span>
            <span
              className={`inspector__field-value${previewTargetLabel === "—" ? " is-muted" : ""}`}
              data-testid="preview-target"
            >
              {previewTargetLabel}
            </span>
          </div>
          {meshSummary ? (
            <div className="inspector__field">
              <span className="inspector__field-label">mesh</span>
              <span
                className="inspector__field-value"
                data-testid="preview-mesh-summary"
              >
                {`${meshSummary.vertices} vertices · ${meshSummary.indices} indices`}
              </span>
            </div>
          ) : null}
        </section>
      </div>
    </aside>
  );
}

type EntryNodeProps = {
  entry: InspectorEntry;
  depth: number;
  onRequestPreview: (entry: InspectorEntry) => void;
  onExpandDirectory: (entry: InspectorEntry) => void;
  onCollapseDirectory: (entry: InspectorEntry) => void;
  expandedDirectories: Map<string, InspectorDirectoryNode>;
  directoryKey: (path: unknown) => string;
};

function EntryNode({
  entry,
  depth,
  onRequestPreview,
  onExpandDirectory,
  onCollapseDirectory,
  expandedDirectories,
  directoryKey,
}: EntryNodeProps) {
  const key = directoryKey(entry.path);
  const isDirectory = entry.kind === "directory";
  const expanded = isDirectory ? expandedDirectories.get(key) : undefined;
  const isExpanded = Boolean(expanded);
  const handleClick = () => {
    if (isDirectory) {
      if (isExpanded) {
        onCollapseDirectory(entry);
      } else {
        onExpandDirectory(entry);
      }
    } else {
      onRequestPreview(entry);
    }
  };
  const indent = depth * 12;
  return (
    <li className="inspector__list-item" role="treeitem" aria-level={depth + 1}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, width: "100%" }}>
        <span aria-hidden="true" style={{ width: indent, display: "inline-block" }} />
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={handleClick}
          data-testid={`entry-${entry.label}`}
          title={isDirectory ? `toggle ${entry.label}` : `preview ${entry.label}`}
          aria-expanded={isDirectory ? isExpanded : undefined}
        >
          {isDirectory ? (isExpanded ? "▾ " : "▸ ") : ""}
          {entry.label}
        </button>
        <span className="dim">{entry.kind}</span>
      </div>
      {isDirectory && expanded ? (
        <ul
          className="inspector__list"
          role="group"
          data-testid={`entries-${key}`}
          style={{ width: "100%" }}
        >
          {expanded.loading ? (
            <li
              className="inspector__list-item"
              data-testid={`entries-${key}-loading`}
            >
              <span className="dim">loading…</span>
            </li>
          ) : expanded.error ? (
            <li
              className="inspector__list-item"
              data-testid={`entries-${key}-error`}
            >
              <span className="dim">{expanded.error}</span>
            </li>
          ) : expanded.entries && expanded.entries.length === 0 ? (
            <li
              className="inspector__list-item"
              data-testid={`entries-${key}-empty`}
            >
              <span className="dim">empty</span>
            </li>
          ) : (
            (expanded.entries ?? []).map((child) => (
              <EntryNode
                key={`${child.label}-${directoryKey(child.path)}`}
                entry={child}
                depth={depth + 1}
                onRequestPreview={onRequestPreview}
                onExpandDirectory={onExpandDirectory}
                onCollapseDirectory={onCollapseDirectory}
                expandedDirectories={expandedDirectories}
                directoryKey={directoryKey}
              />
            ))
          )}
        </ul>
      ) : null}
    </li>
  );
}
